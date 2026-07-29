mod image_compose;
mod nasa_api;
mod os_events;
mod scheduler;
mod settings;
mod store;
mod updater;
mod wallpaper;

use serde::Serialize;
use settings::{FitMode, Mode, Settings};
use std::path::PathBuf;
use std::sync::Arc;
use store::{Applied, Store};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder, Wry};
use tauri_plugin_autostart::ManagerExt;
use tokio::sync::Mutex;

struct AppData {
    settings: Settings,
    settings_path: PathBuf,
    store: Store,
    offline: bool,
    status_message: Option<String>,
    last_check: Option<String>,
}

struct SharedState(Arc<Mutex<AppData>>);
/// Serialises updates: the scheduler and the panel never run one at the same
/// time, and whichever arrives second simply waits.
struct UpdateLock(Mutex<()>);

/// Handles to the tray menu entries whose text changes over time. The tray is
/// read-only: it shows the title and the credits.
struct TrayHandles {
    title: MenuItem<Wry>,
    info: MenuItem<Wry>,
}

/// State pushed to the settings panel (frontend).
#[derive(Clone, Serialize)]
struct UiState {
    mode: Mode,
    fit_mode: FitMode,
    api_key: String,
    specific_date: String,
    offline: bool,
    status_message: Option<String>,
    last_check: Option<String>,
    current: Option<Applied>,
    autostart: bool,
}

fn ui_state(d: &AppData, autostart: bool) -> UiState {
    UiState {
        mode: d.settings.mode,
        fit_mode: d.settings.fit_mode,
        api_key: d.settings.api_key.clone(),
        specific_date: d.settings.specific_date.clone(),
        offline: d.offline,
        status_message: d.status_message.clone(),
        last_check: d.last_check.clone(),
        current: d.store.applied().cloned(),
        autostart,
    }
}

/// Whether the app is registered to start at login. Reading it is a file or
/// registry lookup, so it is only done when the panel state is built.
fn autostart_enabled(app: &AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

async fn current_ui(app: &AppHandle) -> UiState {
    let autostart = autostart_enabled(app);
    let state = app.state::<SharedState>();
    let d = state.0.lock().await;
    ui_state(&d, autostart)
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(3)).collect();
    out.push_str("...");
    out
}

/// Physical resolution of the primary monitor; other monitors get the same
/// image (documented limitation in the README).
///
/// `None` when the platform reports no monitor at all, which happens with the
/// lid closed or the session locked. That is not a resolution change, and the
/// caller must not treat it as one: recomposing for a guessed size would
/// replace a correct wallpaper with a wrong one.
fn screen_size(app: &AppHandle) -> Option<(u32, u32)> {
    let monitor = app.primary_monitor().ok()??;
    let size = monitor.size();
    Some((size.width.max(640), size.height.max(400)))
}

/// Label of the settings panel window, matching `capabilities/default.json`.
const PANEL: &str = "main";

/// Opens the settings panel, creating the window if it does not exist.
///
/// The window is built on demand and destroyed when closed, so no webview
/// process is resident while the app sits in the tray -- which is where it
/// spends essentially all of its life. Nothing is lost when it closes: every
/// setting is persisted by the backend as it is changed.
fn show_panel(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(PANEL) {
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }

    let built = WebviewWindowBuilder::new(app, PANEL, WebviewUrl::default())
        .title("APOD Wallpaper")
        .inner_size(440.0, 640.0)
        .resizable(false)
        .maximizable(false)
        .center()
        .skip_taskbar(true)
        .build();

    match built {
        Ok(window) => {
            let _ = window.set_focus();
        }
        Err(e) => eprintln!("could not open the settings panel: {e}"),
    }
}

/// Pushes the current state to the panel and to the tray menu labels.
async fn refresh_ui(app: &AppHandle) {
    let ui = current_ui(app).await;
    let _ = app.emit("state-updated", &ui);

    if let Some(tray) = app.try_state::<TrayHandles>() {
        let title = ui
            .current
            .as_ref()
            .map(|c| {
                let t = truncate(&c.title, 60);
                if c.media_type == "video" {
                    format!("{t} (video)")
                } else {
                    t
                }
            })
            .unwrap_or_else(|| "No image loaded".to_string());
        let info = ui
            .current
            .as_ref()
            .map(|c| match &c.copyright {
                Some(cr) => format!("{} -- (c) {}", c.date, truncate(cr, 45)),
                None => format!("{} -- NASA (public domain)", c.date),
            })
            .unwrap_or_else(|| "-".to_string());
        let _ = tray.title.set_text(title);
        let _ = tray.info.set_text(info);
    }
}

// ---------------------------------------------------------------------------
// Commands exposed to the panel. Each command waits for the operation to fully
// finish before answering: the frontend blocks its UI meanwhile and shows any
// error. No work is started in the background without its result being
// reported.
// ---------------------------------------------------------------------------

#[tauri::command]
async fn get_state(app: AppHandle) -> Result<UiState, String> {
    Ok(current_ui(&app).await)
}

/// Switches mode and applies it.
///
/// The new mode is persisted before the update runs and is *not* rolled back
/// if it fails: a mode is a standing preference, so a network outage should
/// leave the app aiming at what the user asked for and let the scheduler retry.
/// `set_specific_date` deliberately does the opposite -- see there.
#[tauri::command]
async fn set_mode(app: AppHandle, mode: Mode) -> Result<UiState, String> {
    let changed = {
        let state = app.state::<SharedState>();
        let mut d = state.0.lock().await;
        // Specific-date mode requires a valid date already stored; the panel
        // normally goes through set_specific_date.
        if mode == Mode::Specific {
            updater::validate_apod_date(&d.settings.specific_date)?;
        }
        let changed = d.settings.mode != mode;
        if changed {
            d.settings.mode = mode;
            let path = d.settings_path.clone();
            d.settings.save(&path)?;
        }
        changed
    };
    // Random is the one mode that is not idempotent: asking for it again means
    // "draw another one", so it updates even when the mode did not change.
    // Doing nothing there would leave the button visibly dead.
    if changed || mode == Mode::Random {
        updater::update(&app, true).await?;
    }
    Ok(current_ui(&app).await)
}

#[tauri::command]
async fn set_specific_date(app: AppHandle, date: String) -> Result<UiState, String> {
    let parsed = updater::validate_apod_date(&date)?;
    let previous = {
        let state = app.state::<SharedState>();
        let mut d = state.0.lock().await;
        let previous = (d.settings.mode, d.settings.specific_date.clone());
        d.settings.mode = Mode::Specific;
        d.settings.specific_date = parsed.format("%Y-%m-%d").to_string();
        let path = d.settings_path.clone();
        d.settings.save(&path)?;
        previous
    };

    if let Err(msg) = updater::update(&app, true).await {
        // Day with no publication, network outage...: restore the previous
        // mode so the displayed state stays truthful. The current wallpaper
        // was not touched and the error is shown to the user.
        //
        // Unlike `set_mode`, this does roll back: a date the archive has no
        // entry for is wrong permanently, and retrying it forever would pin
        // the app to a request that can never succeed.
        {
            let state = app.state::<SharedState>();
            let mut d = state.0.lock().await;
            d.settings.mode = previous.0;
            d.settings.specific_date = previous.1;
            let path = d.settings_path.clone();
            let _ = d.settings.save(&path);
        }
        refresh_ui(&app).await;
        return Err(format!(
            "Could not apply the APOD for {}: {msg} The current wallpaper is kept.",
            parsed.format("%d/%m/%Y")
        ));
    }
    Ok(current_ui(&app).await)
}

#[tauri::command]
async fn set_autostart(app: AppHandle, enabled: bool) -> Result<UiState, String> {
    let manager = app.autolaunch();
    let changed = if enabled {
        manager.enable()
    } else {
        manager.disable()
    };
    changed.map_err(|e| format!("Could not change the login item: {e}"))?;
    Ok(current_ui(&app).await)
}

#[tauri::command]
async fn set_fit_mode(app: AppHandle, fit: FitMode) -> Result<UiState, String> {
    {
        let autostart = autostart_enabled(&app);
        let state = app.state::<SharedState>();
        let mut d = state.0.lock().await;
        if d.settings.fit_mode == fit {
            return Ok(ui_state(&d, autostart));
        }
        d.settings.fit_mode = fit;
        let path = d.settings_path.clone();
        d.settings.save(&path)?;
    }
    // Not forced: the image on disk still matches the mode, so this recomposes
    // it locally instead of going back to the API.
    updater::update(&app, false).await?;
    Ok(current_ui(&app).await)
}

/// Saves the key and immediately puts it to use.
///
/// The whole point of typing a key is usually that DEMO_KEY's quota ran out,
/// so saving it and then sitting on the failed state until the next backoff
/// tick would answer the user's problem with a shrug. A failure here leaves
/// the key saved -- it is what the user asked for -- and reports the error.
#[tauri::command]
async fn set_api_key(app: AppHandle, key: String) -> Result<UiState, String> {
    let changed = {
        let state = app.state::<SharedState>();
        let mut d = state.0.lock().await;
        let key = key.trim().to_string();
        let changed = d.settings.api_key != key;
        if changed {
            d.settings.api_key = key;
            let path = d.settings_path.clone();
            d.settings.save(&path)?;
        }
        changed
    };
    if changed {
        updater::update(&app, true).await?;
    }
    Ok(current_ui(&app).await)
}

#[tauri::command]
async fn refresh_now(app: AppHandle) -> Result<UiState, String> {
    updater::update(&app, true).await?;
    Ok(current_ui(&app).await)
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

// ---------------------------------------------------------------------------
// Tray: read-only. Information about the current image, opening the panel,
// quitting. Every setting lives in the panel.
// ---------------------------------------------------------------------------

fn build_tray(app: &tauri::App) -> tauri::Result<()> {
    let title = MenuItem::with_id(app, "title", "Loading...", false, None::<&str>)?;
    let info = MenuItem::with_id(app, "info", "-", false, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", "Open APOD Wallpaper", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &title,
            &info,
            &PredefinedMenuItem::separator(app)?,
            &open,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;

    app.manage(TrayHandles { title, info });

    TrayIconBuilder::with_id("apod-tray")
        .icon(
            app.default_window_icon()
                .expect("missing default icon")
                .clone(),
        )
        .tooltip("APOD Wallpaper")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show_panel(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Tauri's default runtime sizes itself to the machine: ten worker threads
    // on a ten-core laptop, for an app that makes one HTTP request a day. Two
    // is enough. Nothing blocking runs on these threads -- image work and the
    // wallpaper call both go to tokio's separate blocking pool, whose threads
    // exit once idle -- so they only ever shuttle futures along. The runtime
    // lives as long as `run()`, which returns only when the app exits.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("apod-worker")
        .enable_all()
        .build()
        .expect("could not start the async runtime");
    tauri::async_runtime::set(runtime.handle().clone());

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // Second launch: show the panel of the existing instance.
            show_panel(app);
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .invoke_handler(tauri::generate_handler![
            get_state,
            set_mode,
            set_specific_date,
            set_api_key,
            set_fit_mode,
            set_autostart,
            refresh_now,
            quit_app
        ])
        .setup(|app| {
            // No Dock icon: this is a menu-bar application.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let data_dir = app.path().app_data_dir()?;
            let settings_path = data_dir.join("settings.json");
            // Nothing persisted yet: this is the first launch. Worth knowing,
            // because a tray application that starts silently is invisible --
            // and on a GNOME desktop without the AppIndicator extension, there
            // is not even a tray icon to find.
            let first_run = !settings_path.exists();
            let settings = Settings::load(&settings_path);
            let store = Store::load(&data_dir);
            store.ensure_dir().map_err(std::io::Error::other)?;

            // Settings are otherwise only written when one of them changes, so
            // without this the file would stay missing for a user who never
            // touches a setting -- and every launch would look like the first.
            if first_run {
                let _ = settings.save(&settings_path);
            }

            app.manage(UpdateLock(Mutex::new(())));
            app.manage(SharedState(Arc::new(Mutex::new(AppData {
                settings,
                settings_path,
                store,
                offline: false,
                status_message: None,
                last_check: None,
            }))));

            build_tray(app)?;

            // Screen changes and resumes from sleep reach the scheduler
            // through this, which is why it can sleep to the next day change
            // instead of waking up to look for them.
            let wakeup: os_events::Wakeup = Arc::new(tokio::sync::Notify::new());
            os_events::watch(&wakeup);

            // The one and only background task.
            tauri::async_runtime::spawn(scheduler::run(app.handle().clone(), wakeup));

            // Show the panel once, on the very first launch, so the app is not
            // a process the user has no evidence of. Every later start goes
            // straight to the tray.
            if first_run {
                show_panel(app.handle());
            }

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while launching the application")
        .run(|_app, event| {
            // With no visible window, prevent the automatic exit: the app only
            // quits through the tray menu.
            if let tauri::RunEvent::ExitRequested { api, code, .. } = event {
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}
