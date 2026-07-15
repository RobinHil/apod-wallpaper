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
/// Le tray est en lecture seule : il n'affiche que le titre et les credits.
struct TrayHandles {
    title: MenuItem<Wry>,
    info: MenuItem<Wry>,
}

/// Etat envoye au panneau (frontend).
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

/// Messages d'information (non bloquants) accompagnant une image appliquee :
/// APOD du jour pas encore publiee, vignette de video...
fn status_notes(mode: Mode, date: &str, media_type: &str) -> Option<String> {
    let mut notes: Vec<String> = Vec::new();
    if mode == Mode::Daily && date < today_str().as_str() {
        notes.push(format!(
            "L'APOD du jour n'est pas encore publiée — affichage de la plus récente ({date})."
        ));
    }
    if media_type == "video" {
        notes.push(format!(
            "L'APOD du {date} est une vidéo : sa vignette est utilisée en fond d'écran."
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
        .expect("date de depart APOD invalide")
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

/// Analyse et borne une date choisie par l'utilisateur : entre la premiere
/// APOD (16 juin 1995) et aujourd'hui.
fn validate_apod_date(raw: &str) -> Result<NaiveDate, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("Choisissez d'abord une date.".to_string());
    }
    let date = NaiveDate::parse_from_str(raw, "%Y-%m-%d")
        .map_err(|_| format!("Date invalide : « {raw} » (format attendu : AAAA-MM-JJ)."))?;
    let start = apod_start_date();
    if date < start {
        return Err("La première APOD date du 16 juin 1995 : choisissez une date à partir de ce jour.".to_string());
    }
    if date > Local::now().date_naive() {
        return Err("Cette date est dans le futur : choisissez une date passée ou aujourd'hui.".to_string());
    }
    Ok(date)
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

/// Pousse l'etat courant vers le panneau et vers les textes du menu tray.
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
                    format!("{t} (vidéo)")
                } else {
                    t
                }
            })
            .unwrap_or_else(|| "Aucune image chargée".to_string());
        let info = ui
            .current
            .as_ref()
            .map(|c| match &c.copyright {
                Some(cr) => format!("{} — © {}", c.date, truncate(cr, 45)),
                None => format!("{} — NASA (domaine public)", c.date),
            })
            .unwrap_or_else(|| "-".to_string());
        let _ = tray.title.set_text(title);
        let _ = tray.info.set_text(info);
    }
}

/// Point d'entree de toute mise a jour (demarrage, boucle de fond, actions du
/// panneau). Toute erreur est remontee a l'appelant : les commandes du
/// panneau la transmettent au frontend, la boucle de fond la conserve dans
/// le statut (elle est deja enregistree dans l'etat au moment de l'echec).
async fn check_and_update(app: &AppHandle, force: bool) -> Result<(), String> {
    {
        let flag = app.state::<UpdateFlag>();
        if flag.0.swap(true, Ordering::SeqCst) {
            return Err(
                "Une mise à jour est déjà en cours, réessayez dans un instant.".to_string(),
            );
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
        // En mode jour on ne passe pas de date : l'API renvoie la derniere
        // image publiee, ce qui evite tout souci de fuseau horaire.
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

    // Recherche d'une APOD exploitable : une image, ou une video avec
    // vignette (l'API ne fournit pas de fichier video, la vignette en est la
    // seule representation possible en fond d'ecran). En mode aleatoire, un
    // jour sans image exploitable ou sans publication declenche un nouveau
    // tirage.
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
                // Mode jour : media sans aucune image exploitable (video sans
                // vignette...) — on conserve l'image precedente et on le
                // signale (ce n'est pas une erreur).
                let mut d = state.lock().await;
                d.offline = false;
                d.video_skip_date = Some(a.date.clone());
                d.status_message = Some(format!(
                    "L'APOD du {} n'a pas d'image exploitable — image précédente conservée.",
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

    // Deja applique et pas de re-verification forcee : rien a faire, mais on
    // maintient les messages d'information (jour pas encore publie...).
    if !force && current_date.as_deref() == Some(apod.date.as_str()) {
        let mut d = state.lock().await;
        d.offline = false;
        d.status_message = status_notes(mode, &apod.date, &apod.media_type);
        d.last_check = Some(now_stamp());
        return Ok(());
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
            let preferred = apod
                .preferred_download_url()
                .expect("has_image() garantit une URL");
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

/// Compose puis applique une entree du cache en fond d'ecran. La composition
/// n'est recalculee que si le fichier pour cette date et cet ajustement est
/// absent ; sinon le JPEG existant est reapplique tel quel.
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
                .map_err(|e| format!("Lecture de l'image en cache impossible : {e}"))?;
            let composed = image_compose::compose_wallpaper(&img, w, h, fit);
            image_compose::save_jpeg(&composed, &wall)
        })
        .await
        .map_err(|e| format!("Tâche de composition interrompue : {e}"))??;
    }

    wallpaper::set_wallpaper(&wall_path)
}

/// Echec de l'API : passage en mode hors-ligne si pertinent, repli sur le
/// cache local si aucune image n'est encore appliquee, et retour du message
/// destine a l'utilisateur.
async fn handle_failure(
    app: &AppHandle,
    state: &Arc<Mutex<AppData>>,
    err: Option<ApiError>,
    fit: FitMode,
) -> String {
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
                // Date precise : uniquement cette date si elle est en cache.
                // Sinon on ne touche pas au bureau — le fond d'ecran en place
                // (persiste par l'OS) reste celui que l'utilisateur avait.
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
            Some(c) => format!("Hors-ligne — dernière image du {} conservée. {message}", c.date),
            None => format!("Hors-ligne — {message}"),
        }
    } else {
        message
    };
    d.status_message = Some(user_message.clone());
    d.last_check = Some(now_stamp());
    user_message
}

// ---------------------------------------------------------------------------
// Commandes exposees au panneau. Chaque commande attend la fin complete de
// l'operation avant de repondre : le frontend bloque son interface pendant
// ce temps et affiche l'erreur eventuelle. Aucun travail n'est lance en
// arriere-plan sans que son resultat soit remonte.
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
        // Le mode date precise exige une date valide deja enregistree ;
        // le panneau passe normalement par set_specific_date.
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
        // Jour sans publication, panne reseau... : on restaure le mode
        // precedent pour que l'etat affiche reste vrai. Le fond d'ecran
        // actuel n'a pas ete touche, l'erreur est montree a l'utilisateur.
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
            "Impossible d'appliquer l'APOD du {} : {msg} Le fond d'écran actuel est conservé.",
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
        // En mode aleatoire, rafraichir signifie tirer une nouvelle image.
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
// Tray : lecture seule. Informations sur l'image courante, ouverture du
// panneau, sortie. Tous les reglages se font dans le panneau.
// ---------------------------------------------------------------------------

fn build_tray(app: &tauri::App) -> tauri::Result<()> {
    let title = MenuItem::with_id(app, "title", "Chargement...", false, None::<&str>)?;
    let info = MenuItem::with_id(app, "info", "-", false, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", "Ouvrir APOD Wallpaper", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quitter", true, None::<&str>)?;

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
                .expect("icone par defaut absente")
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
            set_specific_date,
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

            // Verification au demarrage puis boucle de fond : nouvelle image
            // quotidienne et reprises silencieuses apres une coupure reseau.
            // Les echecs sont deja consignes dans le statut (tray + panneau).
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let _ = check_and_update(&handle, false).await;

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
                        let _ = check_and_update(&handle, false).await;
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
