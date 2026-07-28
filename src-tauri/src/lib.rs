mod cache;
mod image_compose;
mod nasa_api;
mod settings;
mod wallpaper;

use cache::{Cache, CacheEntry};
use chrono::{Days, Local, NaiveDate};
use nasa_api::{ApiError, Apod};
use serde::Serialize;
use settings::{FitMode, Mode, Settings};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, Wry};
use tokio::sync::Mutex;

/// First APOD ever published: lower bound for the random draw.
const APOD_START: (i32, u32, u32) = (1995, 6, 16);
/// Background loop cadence (daily check plus offline retries).
const CHECK_INTERVAL: Duration = Duration::from_secs(15 * 60);
/// Number of re-draws in random mode when the date lands on a video or a day
/// with no publication.
const MAX_RANDOM_ATTEMPTS: usize = 6;

struct AppData {
    settings: Settings,
    settings_path: PathBuf,
    cache: Cache,
    /// Image currently applied as the wallpaper.
    current: Option<CacheEntry>,
    /// Date drawn at startup (random mode).
    random_date: Option<NaiveDate>,
    /// Date of an APOD video already seen, so we do not re-query the API every
    /// 15 minutes on a day with no image.
    video_skip_date: Option<String>,
    offline: bool,
    status_message: Option<String>,
    last_check: Option<String>,
}

struct SharedState(Arc<Mutex<AppData>>);
struct HttpClient(reqwest::Client);
/// Re-entrancy guard: one update at a time.
struct UpdateFlag(AtomicBool);

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
    using_demo_key: bool,
    specific_date: String,
    offline: bool,
    status_message: Option<String>,
    last_check: Option<String>,
    current: Option<CacheEntry>,
}

fn ui_state(d: &AppData) -> UiState {
    UiState {
        mode: d.settings.mode,
        fit_mode: d.settings.fit_mode,
        api_key: d.settings.api_key.clone(),
        using_demo_key: d.settings.api_key.trim().is_empty(),
        specific_date: d.settings.specific_date.clone(),
        offline: d.offline,
        status_message: d.status_message.clone(),
        last_check: d.last_check.clone(),
        current: d.current.clone(),
    }
}

async fn current_ui(app: &AppHandle) -> UiState {
    let state = app.state::<SharedState>();
    let d = state.0.lock().await;
    ui_state(&d)
}

fn today_str() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

/// Informational, non-blocking notes shown alongside an applied image: today's
/// APOD not published yet, video thumbnail in use...
fn status_notes(mode: Mode, date: &str, media_type: &str) -> Option<String> {
    let mut notes: Vec<String> = Vec::new();
    if mode == Mode::Daily && date < today_str().as_str() {
        notes.push(format!(
            "Today's APOD is not published yet -- showing the most recent one ({date})."
        ));
    }
    if media_type == "video" {
        notes.push(format!(
            "The APOD for {date} is a video: its thumbnail is used as the wallpaper."
        ));
    }
    if notes.is_empty() {
        None
    } else {
        Some(notes.join(" "))
    }
}

fn now_stamp() -> String {
    Local::now().format("%d/%m %H:%M").to_string()
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(3)).collect();
    out.push_str("...");
    out
}

fn apod_start_date() -> NaiveDate {
    NaiveDate::from_ymd_opt(APOD_START.0, APOD_START.1, APOD_START.2)
        .expect("invalid APOD start date")
}

fn pick_random_date() -> NaiveDate {
    use rand::RngExt;
    let start = apod_start_date();
    let today = Local::now().date_naive();
    let span = (today - start).num_days().max(0);
    let offset = rand::rng().random_range(0..=span);
    start
        .checked_add_days(Days::new(offset as u64))
        .unwrap_or(today)
}

/// Parses and bounds a user-chosen date: between the first APOD (16 June 1995)
/// and today.
fn validate_apod_date(raw: &str) -> Result<NaiveDate, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("Pick a date first.".to_string());
    }
    let date = NaiveDate::parse_from_str(raw, "%Y-%m-%d")
        .map_err(|_| format!("Invalid date: \"{raw}\" (expected format: YYYY-MM-DD)."))?;
    let start = apod_start_date();
    if date < start {
        return Err("The first APOD dates from 16 June 1995: pick a date from that day on.".to_string());
    }
    if date > Local::now().date_naive() {
        return Err("That date is in the future: pick today or an earlier date.".to_string());
    }
    Ok(date)
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

/// Entry point for every update (startup, background loop, panel actions).
/// Errors are returned to the caller: panel commands forward them to the
/// frontend, the background loop keeps them in the status (they are already
/// recorded in the state at the point of failure).
async fn check_and_update(app: &AppHandle, force: bool) -> Result<(), String> {
    {
        let flag = app.state::<UpdateFlag>();
        if flag.0.swap(true, Ordering::SeqCst) {
            return Err("An update is already running, try again in a moment.".to_string());
        }
    }
    let result = do_update(app, force).await;
    app.state::<UpdateFlag>().0.store(false, Ordering::SeqCst);
    refresh_ui(app).await;
    result
}

async fn do_update(app: &AppHandle, force: bool) -> Result<(), String> {
    let client = app.state::<HttpClient>().0.clone();
    let state = app.state::<SharedState>().0.clone();

    let (mode, fit, api_key, random_date, specific_raw, current_date) = {
        let mut d = state.lock().await;
        if d.settings.mode == Mode::Random && d.random_date.is_none() {
            d.random_date = Some(pick_random_date());
        }
        (
            d.settings.mode,
            d.settings.fit_mode,
            d.settings.effective_api_key().to_string(),
            d.random_date,
            d.settings.specific_date.clone(),
            d.current.as_ref().map(|c| c.date.clone()),
        )
    };

    let mut target_date = match mode {
        // In daily mode we send no date: the API returns the most recently
        // published image, which sidesteps time-zone issues.
        Mode::Daily => None,
        Mode::Random => random_date,
        Mode::Specific => match validate_apod_date(&specific_raw) {
            Ok(date) => Some(date),
            Err(msg) => {
                let mut d = state.lock().await;
                d.status_message = Some(msg.clone());
                d.last_check = Some(now_stamp());
                return Err(msg);
            }
        },
    };

    // Look for a usable APOD: an image, or a video with a thumbnail (the API
    // serves no video file, so the thumbnail is the only possible wallpaper
    // representation). In random mode, a day with no usable image or no
    // publication triggers a new draw.
    let mut apod: Option<Apod> = None;
    let mut failure: Option<ApiError> = None;
    for _ in 0..MAX_RANDOM_ATTEMPTS {
        match nasa_api::fetch_apod(&client, &api_key, target_date).await {
            Ok(a) if a.has_image() => {
                apod = Some(a);
                break;
            }
            Ok(a) => {
                if mode == Mode::Random {
                    let new_date = pick_random_date();
                    state.lock().await.random_date = Some(new_date);
                    target_date = Some(new_date);
                    continue;
                }
                // Daily mode: media with no usable image (video without a
                // thumbnail...) -- keep the previous image and say so (this is
                // not an error).
                let mut d = state.lock().await;
                d.offline = false;
                d.video_skip_date = Some(a.date.clone());
                d.status_message = Some(format!(
                    "The APOD for {} has no usable image -- previous image kept.",
                    a.date
                ));
                d.last_check = Some(now_stamp());
                return Ok(());
            }
            Err(ApiError::NotFound) if mode == Mode::Random => {
                let new_date = pick_random_date();
                state.lock().await.random_date = Some(new_date);
                target_date = Some(new_date);
                continue;
            }
            Err(e) => {
                failure = Some(e);
                break;
            }
        }
    }

    let Some(apod) = apod else {
        return Err(handle_failure(app, &state, failure, fit).await);
    };

    // Already applied and no forced re-check: nothing to do, but keep the
    // informational notes up to date (day not published yet...).
    if !force && current_date.as_deref() == Some(apod.date.as_str()) {
        let mut d = state.lock().await;
        d.offline = false;
        d.status_message = status_notes(mode, &apod.date, &apod.media_type);
        d.last_check = Some(now_stamp());
        return Ok(());
    }

    // Original image: from the store when possible, otherwise downloaded (HD
    // first, standard URL as a fallback).
    let cached_entry = {
        let d = state.lock().await;
        match d.cache.get(&apod.date) {
            Some(e) if d.cache.image_path(e).exists() => Some(e.clone()),
            _ => None,
        }
    };

    let entry = match cached_entry {
        Some(e) => e,
        None => {
            let preferred = apod
                .preferred_download_url()
                .expect("has_image() guarantees a URL");
            let downloaded = match nasa_api::download_image(&client, &preferred).await {
                Ok(bytes) => Ok((preferred, bytes)),
                Err(first_err) => match apod.fallback_download_url() {
                    Some(fallback) => nasa_api::download_image(&client, &fallback)
                        .await
                        .map(|bytes| (fallback, bytes))
                        .map_err(|_| first_err),
                    None => Err(first_err),
                },
            };
            match downloaded {
                Ok((source_url, bytes)) => {
                    let mut d = state.lock().await;
                    match d.cache.store(&apod, &source_url, &bytes) {
                        Ok(e) => e,
                        Err(msg) => {
                            d.status_message = Some(msg.clone());
                            d.last_check = Some(now_stamp());
                            return Err(msg);
                        }
                    }
                }
                Err(e) => {
                    return Err(handle_failure(app, &state, Some(e), fit).await);
                }
            }
        }
    };

    match apply_entry(app, &state, &entry, fit).await {
        Ok(()) => {
            let mut d = state.lock().await;
            d.status_message = status_notes(mode, &entry.date, &entry.media_type);
            d.current = Some(entry);
            d.offline = false;
            d.video_skip_date = None;
            d.last_check = Some(now_stamp());
            Ok(())
        }
        Err(msg) => {
            let mut d = state.lock().await;
            d.status_message = Some(msg.clone());
            d.last_check = Some(now_stamp());
            Err(msg)
        }
    }
}

/// Composes then applies a stored entry as the wallpaper. The composition is
/// only recomputed when the file for that date and fit mode is missing;
/// otherwise the existing JPEG is re-applied as is.
async fn apply_entry(
    app: &AppHandle,
    state: &Arc<Mutex<AppData>>,
    entry: &CacheEntry,
    fit: FitMode,
) -> Result<(), String> {
    let (image_path, wall_path) = {
        let d = state.lock().await;
        (
            d.cache.image_path(entry),
            d.cache.wallpaper_path(&entry.date, fit),
        )
    };

    if !wall_path.exists() {
        let (w, h) = screen_size(app);
        let wall = wall_path.clone();
        tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
            let img = image::open(&image_path)
                .map_err(|e| format!("Could not read the stored image: {e}"))?;
            let composed = image_compose::compose_wallpaper(&img, w, h, fit);
            image_compose::save_jpeg(&composed, &wall)
        })
        .await
        .map_err(|e| format!("Composition task interrupted: {e}"))??;
    }

    wallpaper::set_wallpaper(&wall_path)
}

/// API failure: switch to offline mode when relevant, fall back to the local
/// store when no image has been applied yet, and return the message meant for
/// the user.
async fn handle_failure(
    app: &AppHandle,
    state: &Arc<Mutex<AppData>>,
    err: Option<ApiError>,
    fit: FitMode,
) -> String {
    let (message, offline) = match err {
        Some(e) => (e.to_string(), e.is_offline()),
        None => (
            "No image found after several draws. Will try again later.".to_string(),
            false,
        ),
    };

    let fallback = {
        let d = state.lock().await;
        if d.current.is_some() {
            None
        } else {
            match d.settings.mode {
                Mode::Random => d.cache.random().cloned(),
                Mode::Daily => d.cache.latest().cloned(),
                // Specific date: only that date if it is in the store.
                // Otherwise leave the desktop alone -- the wallpaper in place
                // (persisted by the OS) stays the one the user had.
                Mode::Specific => d.cache.get(&d.settings.specific_date).cloned(),
            }
        }
    };
    if let Some(entry) = fallback {
        if apply_entry(app, state, &entry, fit).await.is_ok() {
            state.lock().await.current = Some(entry);
        }
    }

    let mut d = state.lock().await;
    d.offline = offline;
    let user_message = if offline {
        match &d.current {
            Some(c) => format!("Offline -- keeping the last image from {}. {message}", c.date),
            None => format!("Offline -- {message}"),
        }
    } else {
        message
    };
    d.status_message = Some(user_message.clone());
    d.last_check = Some(now_stamp());
    user_message
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
            validate_apod_date(&d.settings.specific_date)?;
        }
        let changed = d.settings.mode != mode;
        if changed {
            d.settings.mode = mode;
            if mode == Mode::Random {
                d.random_date = Some(pick_random_date());
            }
            let path = d.settings_path.clone();
            d.settings.save(&path)?;
        }
        changed
    };
    if changed {
        check_and_update(&app, true).await?;
    }
    Ok(current_ui(&app).await)
}

#[tauri::command]
async fn set_specific_date(app: AppHandle, date: String) -> Result<UiState, String> {
    let parsed = validate_apod_date(&date)?;
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

    if let Err(msg) = check_and_update(&app, true).await {
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
    let entry = {
        let state = app.state::<SharedState>();
        let mut d = state.0.lock().await;
        if d.settings.fit_mode == fit {
            return Ok(ui_state(&d));
        }
        d.settings.fit_mode = fit;
        let path = d.settings_path.clone();
        d.settings.save(&path)?;
        d.current.clone()
    };
    if let Some(entry) = entry {
        let state = app.state::<SharedState>().0.clone();
        apply_entry(&app, &state, &entry, fit).await?;
    }
    refresh_ui(&app).await;
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
    {
        let state = app.state::<SharedState>();
        let mut d = state.0.lock().await;
        // In random mode, refreshing means drawing a new image.
        if d.settings.mode == Mode::Random {
            d.random_date = Some(pick_random_date());
        }
    }
    check_and_update(&app, true).await?;
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
            let cache = Cache::load(data_dir.join("cache"));
            cache.ensure_dirs().map_err(std::io::Error::other)?;

            let random_date = if settings.mode == Mode::Random {
                Some(pick_random_date())
            } else {
                None
            };

            app.manage(HttpClient(
                reqwest::Client::builder()
                    .user_agent(concat!("apod-wallpaper/", env!("CARGO_PKG_VERSION")))
                    .connect_timeout(Duration::from_secs(15))
                    .timeout(Duration::from_secs(120))
                    .build()?,
            ));
            app.manage(UpdateFlag(AtomicBool::new(false)));
            app.manage(SharedState(Arc::new(Mutex::new(AppData {
                settings,
                settings_path,
                cache,
                current: None,
                random_date,
                video_skip_date: None,
                offline: false,
                status_message: None,
                last_check: None,
            }))));

            build_tray(app)?;

            // Startup check then background loop: picks up the new daily image
            // and silently recovers after a network outage. Failures are
            // already recorded in the status (tray + panel).
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let _ = check_and_update(&handle, false).await;

                let mut interval = tokio::time::interval(CHECK_INTERVAL);
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                interval.tick().await; // first tick is immediate

                loop {
                    interval.tick().await;
                    let need = {
                        let state = handle.state::<SharedState>();
                        let d = state.0.lock().await;
                        let today = today_str();
                        let stale = d
                            .current
                            .as_ref()
                            .map(|c| c.date.as_str() < today.as_str())
                            .unwrap_or(true);
                        let video_blocked = d
                            .video_skip_date
                            .as_deref()
                            .map(|v| v >= today.as_str())
                            .unwrap_or(false);
                        d.offline
                            || d.current.is_none()
                            || (d.settings.mode == Mode::Daily && stale && !video_blocked)
                    };
                    if need {
                        let _ = check_and_update(&handle, false).await;
                    }
                }
            });

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
