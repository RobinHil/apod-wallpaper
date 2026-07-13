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
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, Wry};
use tokio::sync::Mutex;

/// Premiere APOD publiee : borne basse du tirage aleatoire.
const APOD_START: (i32, u32, u32) = (1995, 6, 16);
/// Cadence de la boucle de fond (verification quotidienne + reprises hors-ligne).
const CHECK_INTERVAL: Duration = Duration::from_secs(15 * 60);
/// Nombre de re-tirages en mode aleatoire quand la date tombe sur une video
/// ou un jour sans publication.
const MAX_RANDOM_ATTEMPTS: usize = 6;

struct AppData {
    settings: Settings,
    settings_path: PathBuf,
    cache: Cache,
    /// Image actuellement appliquee en fond d'ecran.
    current: Option<CacheEntry>,
    /// Date tiree au sort au demarrage (mode aleatoire).
    random_date: Option<NaiveDate>,
    /// Date d'une APOD video deja rencontree, pour ne pas re-interroger
    /// l'API toutes les 15 minutes un jour sans image.
    video_skip_date: Option<String>,
    offline: bool,
    status_message: Option<String>,
    last_check: Option<String>,
}

struct SharedState(Arc<Mutex<AppData>>);
struct HttpClient(reqwest::Client);
/// Garde anti-reentrance : une seule mise a jour a la fois.
struct UpdateFlag(AtomicBool);

/// Poignees vers les entrees du menu tray dont le texte change dynamiquement.
struct TrayHandles {
    title: MenuItem<Wry>,
    info: MenuItem<Wry>,
    excerpt: MenuItem<Wry>,
    status: MenuItem<Wry>,
    mode_daily: CheckMenuItem<Wry>,
    mode_random: CheckMenuItem<Wry>,
}

/// Etat envoye au popup (frontend).
#[derive(Clone, Serialize)]
struct UiState {
    mode: Mode,
    fit_mode: FitMode,
    api_key: String,
    using_demo_key: bool,
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
        offline: d.offline,
        status_message: d.status_message.clone(),
        last_check: d.last_check.clone(),
        current: d.current.clone(),
    }
}

fn today_str() -> String {
    Local::now().format("%Y-%m-%d").to_string()
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

fn pick_random_date() -> NaiveDate {
    use rand::RngExt;
    let start = NaiveDate::from_ymd_opt(APOD_START.0, APOD_START.1, APOD_START.2)
        .expect("date de depart APOD invalide");
    let today = Local::now().date_naive();
    let span = (today - start).num_days().max(0);
    let offset = rand::rng().random_range(0..=span);
    start
        .checked_add_days(Days::new(offset as u64))
        .unwrap_or(today)
}

/// Resolution physique de l'ecran principal ; les autres ecrans recoivent la
/// meme image (limite documentee dans le README).
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

/// Pousse l'etat courant vers le popup et vers les textes du menu tray.
async fn refresh_ui(app: &AppHandle) {
    let ui = {
        let state = app.state::<SharedState>();
        let d = state.0.lock().await;
        ui_state(&d)
    };
    let _ = app.emit("state-updated", &ui);

    if let Some(tray) = app.try_state::<TrayHandles>() {
        let title = ui
            .current
            .as_ref()
            .map(|c| truncate(&c.title, 60))
            .unwrap_or_else(|| "Aucune image chargée".to_string());
        let info = ui
            .current
            .as_ref()
            .map(|c| match &c.copyright {
                Some(cr) => format!("{} — © {}", c.date, truncate(cr, 45)),
                None => format!("{} — NASA (domaine public)", c.date),
            })
            .unwrap_or_else(|| "-".to_string());
        let excerpt = ui
            .current
            .as_ref()
            .map(|c| truncate(&c.explanation, 90))
            .unwrap_or_else(|| "-".to_string());
        let status = match (&ui.status_message, ui.offline) {
            (Some(m), _) => m.clone(),
            (None, true) => match &ui.current {
                Some(c) => format!("Hors-ligne — dernière image du {}", c.date),
                None => "Hors-ligne".to_string(),
            },
            (None, false) => match &ui.last_check {
                Some(t) => format!("À jour (vérifié le {t})"),
                None => "Démarrage...".to_string(),
            },
        };
        let _ = tray.title.set_text(title);
        let _ = tray.info.set_text(info);
        let _ = tray.excerpt.set_text(excerpt);
        let _ = tray.status.set_text(truncate(&status, 90));
        let _ = tray.mode_daily.set_checked(ui.mode == Mode::Daily);
        let _ = tray.mode_random.set_checked(ui.mode == Mode::Random);
    }
}

/// Point d'entree de toute mise a jour (demarrage, boucle de fond, bouton
/// "Rafraîchir maintenant", changement de mode).
async fn check_and_update(app: AppHandle, force: bool) {
    {
        let flag = app.state::<UpdateFlag>();
        if flag.0.swap(true, Ordering::SeqCst) {
            return;
        }
    }
    do_update(&app, force).await;
    app.state::<UpdateFlag>().0.store(false, Ordering::SeqCst);
    refresh_ui(&app).await;
}

async fn do_update(app: &AppHandle, force: bool) {
    let client = app.state::<HttpClient>().0.clone();
    let state = app.state::<SharedState>().0.clone();

    let (mode, fit, api_key, mut target_date, current_date) = {
        let mut d = state.lock().await;
        if d.settings.mode == Mode::Random && d.random_date.is_none() {
            d.random_date = Some(pick_random_date());
        }
        (
            d.settings.mode,
            d.settings.fit_mode,
            d.settings.effective_api_key().to_string(),
            match d.settings.mode {
                // En mode jour on ne passe pas de date : l'API renvoie la
                // derniere image publiee, ce qui evite tout souci de fuseau.
                Mode::Daily => None,
                Mode::Random => d.random_date,
            },
            d.current.as_ref().map(|c| c.date.clone()),
        )
    };

    // Recherche d'une APOD de type image. En mode aleatoire, une video ou un
    // jour sans publication declenche un nouveau tirage.
    let mut apod: Option<Apod> = None;
    let mut failure: Option<ApiError> = None;
    for _ in 0..MAX_RANDOM_ATTEMPTS {
        match nasa_api::fetch_apod(&client, &api_key, target_date).await {
            Ok(a) if a.is_image() => {
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
                // Mode jour : l'APOD est une video, on conserve l'image
                // precedente et on le signale.
                let mut d = state.lock().await;
                d.offline = false;
                d.video_skip_date = Some(a.date.clone());
                d.status_message = Some(format!(
                    "L'APOD du {} est une vidéo — image précédente conservée.",
                    a.date
                ));
                d.last_check = Some(now_stamp());
                return;
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
        handle_failure(app, &state, failure, fit).await;
        return;
    };

    // Deja applique et composition presente : rien a faire.
    let wall_exists = {
        let d = state.lock().await;
        d.cache.wallpaper_path(&apod.date, fit).exists()
    };
    if !force && current_date.as_deref() == Some(apod.date.as_str()) && wall_exists {
        let mut d = state.lock().await;
        d.offline = false;
        d.status_message = None;
        d.last_check = Some(now_stamp());
        return;
    }

    // Image originale : depuis le cache si possible, sinon telechargement
    // (HD en priorite, URL standard en repli).
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
            let hd_url = apod
                .best_image_url()
                .expect("is_image() garantit une URL")
                .to_string();
            let downloaded = match nasa_api::download_image(&client, &hd_url).await {
                Ok(bytes) => Ok((hd_url, bytes)),
                Err(first_err) => match apod.fallback_image_url() {
                    Some(fallback) => nasa_api::download_image(&client, fallback)
                        .await
                        .map(|bytes| (fallback.to_string(), bytes))
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
                            d.status_message = Some(msg);
                            d.last_check = Some(now_stamp());
                            return;
                        }
                    }
                }
                Err(e) => {
                    handle_failure(app, &state, Some(e), fit).await;
                    return;
                }
            }
        }
    };

    match apply_entry(app, &state, &entry, fit, force).await {
        Ok(()) => {
            let mut d = state.lock().await;
            d.current = Some(entry);
            d.offline = false;
            d.video_skip_date = None;
            d.status_message = None;
            d.last_check = Some(now_stamp());
        }
        Err(msg) => {
            let mut d = state.lock().await;
            d.status_message = Some(msg);
            d.last_check = Some(now_stamp());
        }
    }
}

/// Compose (si necessaire) puis applique une entree du cache en fond d'ecran.
async fn apply_entry(
    app: &AppHandle,
    state: &Arc<Mutex<AppData>>,
    entry: &CacheEntry,
    fit: FitMode,
    force_compose: bool,
) -> Result<(), String> {
    let (image_path, wall_path) = {
        let d = state.lock().await;
        (
            d.cache.image_path(entry),
            d.cache.wallpaper_path(&entry.date, fit),
        )
    };

    if force_compose || !wall_path.exists() {
        let (w, h) = screen_size(app);
        let date = entry.date.clone();
        let copyright = entry.copyright.clone();
        let wall = wall_path.clone();
        tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
            let img = image::open(&image_path)
                .map_err(|e| format!("Lecture de l'image en cache impossible : {e}"))?;
            let composed =
                image_compose::compose_wallpaper(&img, w, h, fit, &date, copyright.as_deref());
            image_compose::save_jpeg(&composed, &wall)
        })
        .await
        .map_err(|e| format!("Tâche de composition interrompue : {e}"))??;
    }

    wallpaper::set_wallpaper(&wall_path)
}

/// Echec de l'API : passage en mode hors-ligne si pertinent, et repli sur le
/// cache local si aucune image n'est encore appliquee.
async fn handle_failure(
    app: &AppHandle,
    state: &Arc<Mutex<AppData>>,
    err: Option<ApiError>,
    fit: FitMode,
) {
    let (message, offline) = match err {
        Some(e) => (e.to_string(), e.is_offline()),
        None => (
            "Aucune image trouvée après plusieurs tirages. Nouvel essai plus tard.".to_string(),
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
            }
        }
    };
    if let Some(entry) = fallback {
        if apply_entry(app, state, &entry, fit, false).await.is_ok() {
            state.lock().await.current = Some(entry);
        }
    }

    let mut d = state.lock().await;
    d.offline = offline;
    d.status_message = Some(if offline {
        match &d.current {
            Some(c) => format!("Hors-ligne — dernière image du {}", c.date),
            None => format!("Hors-ligne — {message}"),
        }
    } else {
        message
    });
    d.last_check = Some(now_stamp());
}

async fn change_mode(app: AppHandle, mode: Mode) -> Result<(), String> {
    let changed = {
        let state = app.state::<SharedState>();
        let mut d = state.0.lock().await;
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
        check_and_update(app, true).await;
    } else {
        // Re-synchronise les coches du menu (un clic les bascule visuellement).
        refresh_ui(&app).await;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Commandes exposees au popup
// ---------------------------------------------------------------------------

#[tauri::command]
async fn get_state(state: tauri::State<'_, SharedState>) -> Result<UiState, String> {
    let d = state.0.lock().await;
    Ok(ui_state(&d))
}

#[tauri::command]
async fn set_mode(app: AppHandle, mode: Mode) -> Result<(), String> {
    tauri::async_runtime::spawn(async move {
        let _ = change_mode(app, mode).await;
    });
    Ok(())
}

#[tauri::command]
async fn set_api_key(app: AppHandle, key: String) -> Result<(), String> {
    let need_retry = {
        let state = app.state::<SharedState>();
        let mut d = state.0.lock().await;
        d.settings.api_key = key.trim().to_string();
        let path = d.settings_path.clone();
        d.settings.save(&path)?;
        // Une nouvelle cle peut debloquer un quota depasse.
        d.offline || d.current.is_none()
    };
    refresh_ui(&app).await;
    if need_retry {
        tauri::async_runtime::spawn(check_and_update(app, false));
    }
    Ok(())
}

#[tauri::command]
async fn set_fit_mode(app: AppHandle, fit: FitMode) -> Result<(), String> {
    let current = {
        let state = app.state::<SharedState>();
        let mut d = state.0.lock().await;
        if d.settings.fit_mode == fit {
            return Ok(());
        }
        d.settings.fit_mode = fit;
        let path = d.settings_path.clone();
        d.settings.save(&path)?;
        d.current.clone()
    };
    if let Some(entry) = current {
        let state = app.state::<SharedState>().0.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(msg) = apply_entry(&app, &state, &entry, fit, false).await {
                state.lock().await.status_message = Some(msg);
            }
            refresh_ui(&app).await;
        });
    }
    Ok(())
}

#[tauri::command]
async fn refresh_now(app: AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn(check_and_update(app, true));
    Ok(())
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

// ---------------------------------------------------------------------------
// Tray
// ---------------------------------------------------------------------------

fn build_tray(app: &tauri::App, initial_mode: Mode) -> tauri::Result<()> {
    let title = MenuItem::with_id(app, "title", "Chargement...", false, None::<&str>)?;
    let info = MenuItem::with_id(app, "info", "-", false, None::<&str>)?;
    let excerpt = MenuItem::with_id(app, "excerpt", "-", false, None::<&str>)?;
    let status = MenuItem::with_id(app, "status", "Démarrage...", false, None::<&str>)?;
    let mode_daily = CheckMenuItem::with_id(
        app,
        "mode_daily",
        "Mode image du jour",
        true,
        initial_mode == Mode::Daily,
        None::<&str>,
    )?;
    let mode_random = CheckMenuItem::with_id(
        app,
        "mode_random",
        "Mode aléatoire",
        true,
        initial_mode == Mode::Random,
        None::<&str>,
    )?;
    let refresh = MenuItem::with_id(app, "refresh", "Rafraîchir maintenant", true, None::<&str>)?;
    let panel = MenuItem::with_id(app, "panel", "Détails et réglages...", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quitter", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &title,
            &info,
            &excerpt,
            &PredefinedMenuItem::separator(app)?,
            &mode_daily,
            &mode_random,
            &PredefinedMenuItem::separator(app)?,
            &refresh,
            &panel,
            &PredefinedMenuItem::separator(app)?,
            &status,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;

    app.manage(TrayHandles {
        title,
        info,
        excerpt,
        status,
        mode_daily,
        mode_random,
    });

    TrayIconBuilder::with_id("apod-tray")
        .icon(
            app.default_window_icon()
                .expect("icone par defaut absente")
                .clone(),
        )
        .tooltip("APOD Wallpaper")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "mode_daily" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = change_mode(app, Mode::Daily).await;
                });
            }
            "mode_random" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = change_mode(app, Mode::Random).await;
                });
            }
            "refresh" => {
                let app = app.clone();
                tauri::async_runtime::spawn(check_and_update(app, true));
            }
            "panel" => show_panel(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Entree
// ---------------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // Deuxieme lancement : on montre le panneau de l'instance existante.
            show_panel(app);
        }))
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_state,
            set_mode,
            set_api_key,
            set_fit_mode,
            refresh_now,
            quit_app
        ])
        .on_window_event(|window, event| {
            // Fermer le panneau le cache : l'application vit dans le tray.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .setup(|app| {
            // Pas d'icone dans le Dock : application de barre de menus.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let data_dir = app.path().app_data_dir()?;
            let settings_path = data_dir.join("settings.json");
            let settings = Settings::load(&settings_path);
            let cache = Cache::load(data_dir.join("cache"));
            cache.ensure_dirs().map_err(std::io::Error::other)?;

            let initial_mode = settings.mode;
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

            build_tray(app, initial_mode)?;

            // Verification au demarrage puis boucle de fond : nouvelle image
            // quotidienne et reprises silencieuses apres une coupure reseau.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                check_and_update(handle.clone(), false).await;

                let mut interval = tokio::time::interval(CHECK_INTERVAL);
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                interval.tick().await; // premier tick immediat

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
                        check_and_update(handle.clone(), false).await;
                    }
                }
            });

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("erreur au lancement de l'application")
        .run(|_app, event| {
            // Sans fenetre visible, on empeche la sortie automatique :
            // l'application ne quitte que via le menu tray.
            if let tauri::RunEvent::ExitRequested { api, code, .. } = event {
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}
