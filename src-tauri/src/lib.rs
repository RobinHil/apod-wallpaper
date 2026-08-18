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

/// Handles to the menu bar entries whose text changes over time. That menu is
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
}

fn ui_state(d: &AppData) -> UiState {
    UiState {
        mode: d.settings.mode,
        fit_mode: d.settings.fit_mode,
        api_key: d.settings.api_key.clone(),
        specific_date: d.settings.specific_date.clone(),
        offline: d.offline,
        status_message: d.status_message.clone(),
        last_check: d.last_check.clone(),
        current: d.store.applied().cloned(),
    }
}

async fn current_ui(app: &AppHandle) -> UiState {
    let state = app.state::<SharedState>();
    let d = state.0.lock().await;
    ui_state(&d)
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(3)).collect();
    out.push_str("...");
    out
}

/// Physical resolution of the monitor the wallpaper is composed for -- the
/// main display, the one carrying the menu bar. The others get the same image
/// (documented limitation in the README).
///
/// `None` when macOS reports no main display at all, which happens with the
/// lid closed and no external screen attached. That is not a resolution
/// change, and the caller must not treat it as one: recomposing for a guessed
/// size would replace a correct wallpaper with a wrong one.
fn screen_size(app: &AppHandle) -> Option<(u32, u32)> {
    let monitor = app.primary_monitor().ok()??;
    let size = monitor.size();
    let (min_w, min_h) = image_compose::MIN_SCREEN;
    Some((size.width.max(min_w), size.height.max(min_h)))
}

/// Label of the settings panel window, matching `capabilities/default.json`.
const PANEL: &str = "main";

/// Opens the settings panel, creating the window if it does not exist.
///
/// The window is built on demand and destroyed when closed, so no webview
/// process is resident while the app sits in the background -- which is where
/// it spends essentially all of its life. Nothing is lost when it closes:
/// every setting is persisted by the backend as it is changed.
fn show_panel(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(PANEL) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        return;
    }

    let built = WebviewWindowBuilder::new(app, PANEL, WebviewUrl::default())
        .title("APOD Wallpaper")
        .inner_size(440.0, 640.0)
        .resizable(false)
        .maximizable(false)
        .center()
        // Hidden from the app switcher: the menu bar is the way back to a
        // background utility, and the activation policy set at startup keeps
        // it out of the Dock anyway.
        .skip_taskbar(true)
        .build();

    match built {
        Ok(window) => {
            let _ = window.set_focus();
        }
        Err(e) => eprintln!("could not open the settings panel: {e}"),
    }
}

/// Pushes the current state to the panel and to the menu bar labels.
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
            d.settings.save(&d.settings_path)?;
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
        d.settings.save(&d.settings_path)?;
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
            let _ = d.settings.save(&d.settings_path);
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
async fn set_fit_mode(app: AppHandle, fit: FitMode) -> Result<UiState, String> {
    {
        let state = app.state::<SharedState>();
        let mut d = state.0.lock().await;
        if d.settings.fit_mode == fit {
            return Ok(ui_state(&d));
        }
        d.settings.fit_mode = fit;
        d.settings.save(&d.settings_path)?;
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
            d.settings.save(&d.settings_path)?;
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
// Menu bar item: read-only. Information about the current image, opening the
// panel, quitting. Every setting lives in the panel.
//
// It is a convenience, never the only way in. Nothing depends on it: the panel
// carries every setting, the manual refresh and the quit button, launching the
// app again brings that panel up (single instance), and an item that fails to
// build is logged and stepped over rather than being a startup error.
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

    // Reported rather than asserted: the caller treats a menu bar item it
    // could not build as a missing convenience and starts anyway, which a
    // panic here would turn back into a fatal error.
    let icon = app
        .default_window_icon()
        .ok_or_else(|| {
            tauri::Error::Io(std::io::Error::other(
                "the bundle carries no default icon to put in the menu bar",
            ))
        })?
        .clone();

    TrayIconBuilder::with_id("apod-tray")
        .icon(icon)
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

/// What the binary answers to `--help`. Short on purpose: this application has
/// no options that change how it behaves. Starting it never puts anything on
/// screen, so there is no longer a flag asking it not to.
const USAGE: &str = concat!(
    "APOD Wallpaper ",
    env!("CARGO_PKG_VERSION"),
    "\n\nSets NASA's Astronomy Picture of the Day as your desktop wallpaper.\n\
     Starts in the background and stays there; the menu bar icon opens the\n\
     settings panel.\n\n\
     Usage: apod-wallpaper [OPTIONS]\n\n\
     Options:\n  \
       -h, --help    Show this message\n  \
       -V, --version Show the version\n"
);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "-h" | "--help" => return println!("{USAGE}"),
            "-V" | "--version" => return println!(env!("CARGO_PKG_VERSION")),
            _ => {}
        }
    }

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
            // A second *process*, which only happens when the binary inside
            // the bundle is run directly. Launching the bundle again while it
            // runs starts nothing: macOS sends a reopen event instead, handled
            // in `run()`. Both are deliberate, and both open the panel.
            show_panel(app);
        }))
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_state,
            set_mode,
            set_specific_date,
            set_api_key,
            set_fit_mode,
            refresh_now,
            quit_app
        ])
        .setup(move |app| {
            // No Dock icon and no application menu: this is a menu-bar
            // application. `LSUIElement` in `Info.plist` says the same for a
            // bundled build; this covers the binary run on its own.
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let data_dir = app.path().app_data_dir()?;
            let settings_path = data_dir.join("settings.json");
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

            // A menu bar item that cannot be built is a missing convenience,
            // not a reason to refuse to start: see the note above `build_tray`.
            if let Err(e) = build_tray(app) {
                eprintln!("no menu bar icon: {e}");
            }

            // Screen changes and resumes from sleep reach the scheduler
            // through this, which is why it can sleep to the next day change
            // instead of waking up to look for them.
            let wakeup: os_events::Wakeup = Arc::new(tokio::sync::Notify::new());
            os_events::watch(&wakeup);

            // The one and only background task.
            tauri::async_runtime::spawn(scheduler::run(app.handle().clone(), wakeup));

            // Starting the app is not a request to see it. It lives in the
            // menu bar, it is started at login, and a login that throws a
            // window at the screen is exactly what a background utility must
            // not do -- so an ordinary start puts nothing on screen at all.
            //
            // The very first launch is the one exception. There is no menu bar
            // icon the user has learnt to look for yet and no wallpaper set,
            // so opening the panel once is how the app says where it went.
            // Every later start, login included, is silent.
            if first_run {
                show_panel(app.handle());
            }

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while launching the application")
        .run(|app, event| match event {
            // Launched again from the Finder, Spotlight, Launchpad or `open`
            // while already running. macOS starts no second process for a
            // bundled app -- it sends this instead -- so this, and not the
            // single-instance plugin, is what makes launching the app again
            // bring the panel back.
            tauri::RunEvent::Reopen { .. } => show_panel(app),
            // With no visible window, prevent the automatic exit: the app only
            // quits through the panel or the menu bar.
            tauri::RunEvent::ExitRequested { api, code, .. } if code.is_none() => {
                api.prevent_exit()
            }
            _ => {}
        });
}

#[cfg(test)]
mod tests {
    use super::{truncate, USAGE};

    #[test]
    fn the_usage_text_matches_the_options_that_are_parsed() {
        assert!(USAGE.contains("--help"));
        assert!(USAGE.contains("--version"));
        // Starting quietly is the only behaviour now, not something a flag
        // asks for. A `--background` left in the help text would send users
        // to a launch agent they no longer need.
        assert!(!USAGE.contains("--background"));
    }

    #[test]
    fn truncate_keeps_short_titles_and_bounds_long_ones() {
        assert_eq!(truncate("Andromeda", 20), "Andromeda");
        assert_eq!(truncate("Andromeda", 6), "And...");
        // Counted in characters, not bytes: a title cut mid-character would
        // reach the menu bar as broken text.
        assert_eq!(truncate("éééééé", 4), "é...");
    }
}
