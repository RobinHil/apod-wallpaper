mod commands;
mod image_compose;
mod memory;
mod nasa_api;
mod os_events;
mod panel;
mod scheduler;
mod screen;
mod settings;
mod store;
mod tray;
mod updater;
mod video_frame;
mod wallpaper;

use serde::Serialize;
use settings::{FitMode, Mode, Settings};
use std::path::PathBuf;
use std::sync::Arc;
use store::{Applied, Store};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex;

pub struct AppData {
    pub settings: Settings,
    pub settings_path: PathBuf,
    pub store: Store,
    pub offline: bool,
    pub status_message: Option<String>,
    pub last_check: Option<String>,
}

pub struct SharedState(Arc<Mutex<AppData>>);
/// Serialises updates: the scheduler and the panel never run one at the same
/// time, and whichever arrives second simply waits.
pub struct UpdateLock(Mutex<()>);

/// State pushed to the settings panel (frontend).
#[derive(Clone, Serialize)]
pub struct UiState {
    pub mode: Mode,
    pub fit_mode: FitMode,
    pub api_key: String,
    pub specific_date: String,
    pub offline: bool,
    pub status_message: Option<String>,
    pub last_check: Option<String>,
    pub current: Option<Applied>,
}

pub(crate) fn ui_state(d: &AppData) -> UiState {
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

pub async fn current_ui(app: &AppHandle) -> UiState {
    let state = app.state::<SharedState>();
    let d = state.0.lock().await;
    ui_state(&d)
}

/// Pushes the current state to the panel and to the tray labels.
pub(crate) async fn refresh_ui(app: &AppHandle) {
    let ui = current_ui(app).await;
    let _ = app.emit("state-updated", &ui);
    tray::update_labels(app, &ui);
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
     Starts in the background and stays there; the tray icon opens the\n\
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
            panel::show_panel(app);
        }))
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::set_mode,
            commands::set_specific_date,
            commands::set_api_key,
            commands::set_fit_mode,
            commands::refresh_now,
            commands::quit_app
        ])
        .setup(move |app| {
            // No Dock icon and no application menu: this is a menu-bar
            // application. `LSUIElement` in `Info.plist` says the same for a
            // bundled build; this covers the binary run on its own.
            //
            // There is nothing to do here on GNOME. An application with no
            // window open occupies nothing in the Shell already, and its
            // `.desktop` entry deliberately stays visible in the app grid:
            // without a tray icon, launching the application again is the only
            // way back to the panel.
            #[cfg(target_os = "macos")]
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

            // A tray item that cannot be built is a missing convenience,
            // not a reason to refuse to start: see the note above `build_tray`.
            if let Err(e) = tray::build(app) {
                eprintln!("no tray icon: {e}");
            }

            // Screen changes and resumes from sleep reach the scheduler
            // through this, which is why it can sleep to the next day change
            // instead of waking up to look for them.
            let wakeup: os_events::Wakeup = Arc::new(tokio::sync::Notify::new());
            os_events::watch(&wakeup);

            // The one and only background task.
            tauri::async_runtime::spawn(scheduler::run(app.handle().clone(), wakeup));

            // Starting the app is not a request to see it. It lives in the
            // tray, it is started at login, and a login that throws a
            // window at the screen is exactly what a background utility must
            // not do -- so an ordinary start puts nothing on screen at all.
            //
            // The very first launch is the one exception. There is no tray
            // icon the user has learnt to look for yet and no wallpaper set,
            // so opening the panel once is how the app says where it went.
            // Every later start, login included, is silent.
            if first_run {
                panel::show_panel(app.handle());
            }

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while launching the application")
        // `_app` because the only arm that reads it is the macOS one below, and
        // an ordinary name would warn on every other platform.
        .run(|_app, event| match event {
            // Launched again from the Finder, Spotlight, Launchpad or `open`
            // while already running. macOS starts no second process for a
            // bundled app -- it sends this instead -- so this, and not the
            // single-instance plugin, is what makes launching the app again
            // bring the panel back.
            //
            // The event is macOS's alone. GNOME starts a second process, which
            // the single-instance plugin hands to the running one, so both
            // desktops end in the same place by different routes.
            #[cfg(target_os = "macos")]
            tauri::RunEvent::Reopen { .. } => panel::show_panel(_app),
            // With no visible window, prevent the automatic exit: the app only
            // quits through the panel or the tray.
            tauri::RunEvent::ExitRequested { api, code, .. } if code.is_none() => {
                api.prevent_exit()
            }
            _ => {}
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Field names serde emits for a type, which is what actually crosses to
    /// the panel -- not the Rust names, and not what anyone remembered.
    fn emitted_fields<T: Serialize>(value: &T) -> Vec<String> {
        match serde_json::to_value(value).expect("the panel contract must serialise") {
            serde_json::Value::Object(map) => map.keys().cloned().collect(),
            other => panic!("expected an object, got {other}"),
        }
    }

    /// Field names declared by one `export interface` block of `types.ts`.
    ///
    /// A deliberately small parser. Generating the TypeScript from the Rust
    /// would remove the need for it, and would also throw away the comments
    /// that make `types.ts` worth reading; this keeps both files hand-written
    /// and makes disagreeing between them a build failure.
    fn declared_fields(interface: &str) -> Vec<String> {
        let source = include_str!("../../src/types.ts");
        let start = source
            .find(&format!("export interface {interface} {{"))
            .unwrap_or_else(|| panic!("types.ts declares no interface {interface}"));
        let body = &source[start..];
        let end = body.find("\n}").expect("unterminated interface");

        let mut fields: Vec<String> = body[..end]
            .lines()
            .skip(1)
            .filter_map(|line| {
                let line = line.trim();
                // Skip comments and blank lines; a field is `name: type;` or
                // `name?: type;`, and only the name before the colon matters.
                if line.is_empty()
                    || line.starts_with("//")
                    || line.starts_with('*')
                    || line.starts_with("/*")
                {
                    return None;
                }
                let (name, _) = line.split_once(':')?;
                Some(name.trim().trim_end_matches('?').to_string())
            })
            .collect();
        fields.sort();
        fields
    }

    /// The one thing in this repository that nothing else checks.
    ///
    /// `UiState` is the whole contract with the panel, and it is written twice:
    /// here, and by hand in `src/types.ts`. Adding a field on one side and
    /// forgetting the other breaks neither the build, nor the typecheck, nor
    /// any other test -- it breaks the panel at runtime, silently, on a user's
    /// machine. So it breaks this instead.
    #[test]
    fn the_panel_sees_exactly_the_fields_this_sends() {
        let applied = Applied {
            date: "2026-07-28".into(),
            title: "t".into(),
            explanation: "e".into(),
            copyright: None,
            media_type: "image".into(),
            video_url: None,
            source_url: "u".into(),
            image_file: "i.jpg".into(),
            wallpaper_file: "w.jpg".into(),
            fit: FitMode::BlurFill,
            width: 1,
            height: 1,
            applied_on: "2026-07-28".into(),
        };
        let state = UiState {
            mode: Mode::Daily,
            fit_mode: FitMode::BlurFill,
            api_key: String::new(),
            specific_date: String::new(),
            offline: false,
            status_message: None,
            last_check: None,
            current: Some(applied.clone()),
        };

        assert_eq!(
            emitted_fields(&state),
            declared_fields("UiState"),
            "UiState and src/types.ts disagree"
        );

        // `Applied` reaches the panel inside `UiState.current`, and the panel
        // reads more of it than of anything else. It is allowed to declare
        // fewer fields than the backend sends -- serde emits the whole record,
        // including the file names the panel has no use for -- but never a
        // field the backend does not send.
        let sent = emitted_fields(&applied);
        for field in declared_fields("Applied") {
            assert!(
                sent.contains(&field),
                "src/types.ts declares Applied.{field}, which the backend never sends"
            );
        }
    }

    /// The two enums cross as strings, and the strings are spelled out again
    /// in `types.ts` as union members.
    #[test]
    fn the_panel_spells_the_enum_values_the_way_serde_does() {
        let source = include_str!("../../src/types.ts");
        for mode in [Mode::Daily, Mode::Random, Mode::Specific] {
            let emitted = serde_json::to_string(&mode).expect("a mode must serialise");
            assert!(
                source.contains(&emitted),
                "src/types.ts never mentions the mode {emitted}"
            );
        }
        for fit in [FitMode::BlurFill, FitMode::CropFill] {
            let emitted = serde_json::to_string(&fit).expect("a fit mode must serialise");
            assert!(
                source.contains(&emitted),
                "src/types.ts never mentions the fit mode {emitted}"
            );
        }
    }

    #[test]
    fn the_usage_text_matches_the_options_that_are_parsed() {
        assert!(USAGE.contains("--help"));
        assert!(USAGE.contains("--version"));
        // Starting quietly is the only behaviour now, not something a flag
        // asks for. A `--background` left in the help text would send users
        // to a launch agent they no longer need.
        assert!(!USAGE.contains("--background"));
    }
}
