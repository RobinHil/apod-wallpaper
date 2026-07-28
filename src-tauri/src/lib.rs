mod image_compose;
mod nasa_api;
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
use tauri::{AppHandle, Emitter, Manager, Wry};
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

/// Physical resolution of the primary monitor; other monitors get the same
/// image (documented limitation in the README).
fn screen_size(app: &AppHandle) -> (u32, u32) {
    match app.primary_monitor() {
        Ok(Some(monitor)) => {
            let size = monitor.size();
            (size.width.max(640), size.height.max(400))
        }
        _ => (1920, 1080),
    }
}

fn show_panel(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
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
async fn get_state(state: tauri::State<'_, SharedState>) -> Result<UiState, String> {
    let d = state.0.lock().await;
    Ok(ui_state(&d))
}

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
    if changed {
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
async fn set_fit_mode(app: AppHandle, fit: FitMode) -> Result<UiState, String> {
    {
        let state = app.state::<SharedState>();
        let mut d = state.0.lock().await;
        if d.settings.fit_mode == fit {
            return Ok(ui_state(&d));
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

#[tauri::command]
async fn set_api_key(app: AppHandle, key: String) -> Result<UiState, String> {
    {
        let state = app.state::<SharedState>();
        let mut d = state.0.lock().await;
        d.settings.api_key = key.trim().to_string();
        let path = d.settings_path.clone();
        d.settings.save(&path)?;
    }
    refresh_ui(&app).await;
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
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // Second launch: show the panel of the existing instance.
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
        .on_window_event(|window, event| {
            // Closing the panel hides it: the app lives in the tray.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .setup(|app| {
            // No Dock icon: this is a menu-bar application.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let data_dir = app.path().app_data_dir()?;
            let settings_path = data_dir.join("settings.json");
            let settings = Settings::load(&settings_path);
            let store = Store::load(&data_dir);
            store.ensure_dir().map_err(std::io::Error::other)?;

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

            // The one and only background task.
            tauri::async_runtime::spawn(scheduler::run(app.handle().clone()));

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
