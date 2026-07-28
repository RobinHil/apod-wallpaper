use crate::nasa_api::{self, ApiError, Apod};
use crate::settings::{FitMode, Mode, Settings};
use crate::store::Applied;
use crate::{image_compose, wallpaper, AppData, SharedState, UpdateLock};
use chrono::{Days, Local, NaiveDate};
use std::fs;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tokio::sync::Mutex;

/// First APOD ever published: lower bound for the random draw.
const APOD_START: (i32, u32, u32) = (1995, 6, 16);
/// Re-draws allowed in random mode when a date lands on a day with no
/// publication or no usable image.
const MAX_RANDOM_ATTEMPTS: usize = 6;

/// What the scheduler should do once an update returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Nothing left to do until the local day changes.
    Satisfied,
    /// Today's APOD is not published yet. Worth another look later, but there
    /// is no hurry: this is not a failure.
    AwaitingPublication,
}

type State = Arc<Mutex<AppData>>;

/// Runs an update, serialised against every other update. `force` skips the
/// "is anything due?" check and is used by the panel actions.
///
/// Failures leave the desktop untouched: the wallpaper already in place stays,
/// and the caller decides whether to surface the message or retry later.
pub async fn update(app: &AppHandle, force: bool) -> Result<Outcome, String> {
    let lock = app.state::<UpdateLock>();
    let _guard = lock.0.lock().await;
    let state = app.state::<SharedState>().0.clone();

    let result = run(app, &state, force).await;

    {
        let mut d = state.lock().await;
        d.last_check = Some(Local::now().format("%d/%m %H:%M").to_string());
        if let Err(msg) = &result {
            d.status_message = Some(msg.clone());
        }
    }
    crate::refresh_ui(app).await;
    result
}

async fn run(app: &AppHandle, state: &State, force: bool) -> Result<Outcome, String> {
    let (settings, applied) = {
        let d = state.lock().await;
        (d.settings.clone(), d.store.applied().cloned())
    };
    let today = today_string();
    let (width, height) = crate::screen_size(app);

    // Nothing due: the image on disk already answers the current settings.
    if !force {
        if let Some(a) = &applied {
            let usable = { state.lock().await.store.files_present(a) };
            if usable && on_target(&settings, a, &today) {
                if a.fit == settings.fit_mode && a.width == width && a.height == height {
                    finish(state, note(&settings, a, &today)).await;
                    return Ok(Outcome::Satisfied);
                }
                // Right image, wrong screen size or fit mode: recompose from
                // the stored original, no network involved.
                reapply(state, a.clone(), settings.fit_mode, width, height).await?;
                finish(state, note(&settings, a, &today)).await;
                return Ok(Outcome::Satisfied);
            }
        }
    }

    let client = http_client()?;
    let api_key = settings.effective_api_key().to_string();
    let mut target = match settings.mode {
        // Daily mode sends no date: the API returns the most recently
        // published image, which sidesteps time-zone skew.
        Mode::Daily => None,
        Mode::Random => Some(pick_random_date()),
        Mode::Specific => Some(validate_apod_date(&settings.specific_date)?),
    };

    let mut found: Option<Apod> = None;
    for _ in 0..MAX_RANDOM_ATTEMPTS {
        match nasa_api::fetch_apod(&client, &api_key, target).await {
            Ok(apod) if apod.has_image() => {
                found = Some(apod);
                break;
            }
            // A real publication with no usable image (a video with no
            // thumbnail). Random mode simply draws again.
            Ok(apod) => {
                if settings.mode == Mode::Random {
                    target = Some(pick_random_date());
                    continue;
                }
                let message = format!(
                    "The APOD for {} has no usable image -- current wallpaper kept.",
                    apod.date
                );
                finish(state, Some(message)).await;
                return Ok(if settings.mode == Mode::Daily && apod.date < today {
                    Outcome::AwaitingPublication
                } else {
                    Outcome::Satisfied
                });
            }
            Err(ApiError::NotFound) if settings.mode == Mode::Random => {
                target = Some(pick_random_date());
                continue;
            }
            Err(e) => return Err(offline(state, e).await),
        }
    }

    let Some(apod) = found else {
        return Err(fail(
            state,
            "No usable image found after several random draws. Will try again later.".to_string(),
        )
        .await);
    };

    // The API returned the image already applied. Typical in daily mode just
    // after the local day change, before the new APOD is published.
    if let Some(a) = &applied {
        let usable = { state.lock().await.store.files_present(a) };
        if usable && a.date == apod.date {
            if a.fit != settings.fit_mode || a.width != width || a.height != height {
                reapply(state, a.clone(), settings.fit_mode, width, height).await?;
            }
            finish(state, note(&settings, a, &today)).await;
            return Ok(if settings.mode == Mode::Daily && a.date < today {
                Outcome::AwaitingPublication
            } else {
                Outcome::Satisfied
            });
        }
    }

    let url = apod
        .preferred_download_url()
        .expect("has_image() guarantees a URL");
    let downloaded = match nasa_api::download_image(&client, &url).await {
        Ok(bytes) => Ok((url, bytes)),
        Err(first) => match apod.fallback_download_url() {
            Some(fallback) => nasa_api::download_image(&client, &fallback)
                .await
                .map(|bytes| (fallback, bytes))
                .map_err(|_| first),
            None => Err(first),
        },
    };
    let (source_url, bytes) = match downloaded {
        Ok(v) => v,
        Err(e) => return Err(offline(state, e).await),
    };

    let record = install(
        state,
        &apod,
        source_url,
        bytes,
        settings.fit_mode,
        width,
        height,
    )
    .await?;

    finish(state, note(&settings, &record, &today)).await;
    Ok(if settings.mode == Mode::Daily && record.date < today {
        Outcome::AwaitingPublication
    } else {
        Outcome::Satisfied
    })
}

/// Is the applied image the one the current settings ask for right now?
fn on_target(settings: &Settings, a: &Applied, today: &str) -> bool {
    match settings.mode {
        Mode::Daily => a.date == today,
        Mode::Random => a.applied_on == today,
        Mode::Specific => a.date == settings.specific_date,
    }
}

/// Writes the downloaded payload to a temporary file, decodes it (which
/// is the validity check), composes the wallpaper, and only then moves both
/// files into place with atomic renames. The previous image stays untouched
/// until the new one has actually been applied.
async fn install(
    state: &State,
    apod: &Apod,
    source_url: String,
    bytes: Vec<u8>,
    fit: FitMode,
    width: u32,
    height: u32,
) -> Result<Applied, String> {
    let dir = {
        let d = state.lock().await;
        d.store.ensure_dir()?;
        d.store.dir().to_path_buf()
    };

    let image_file = format!("{}.{}", apod.date, image_extension(&bytes));
    let wallpaper_file = wallpaper_file_name(&apod.date, fit, width, height);
    let tmp_image = dir.join(".incoming-image");
    let tmp_wallpaper = dir.join(".incoming-wallpaper");
    let final_image = dir.join(&image_file);
    let final_wallpaper = dir.join(&wallpaper_file);

    let paths = (
        tmp_image.clone(),
        tmp_wallpaper.clone(),
        final_image,
        final_wallpaper.clone(),
    );
    let composed = tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let (tmp_image, tmp_wallpaper, final_image, final_wallpaper) = paths;
        fs::write(&tmp_image, &bytes)
            .map_err(|e| format!("Could not write the downloaded image: {e}"))?;
        // Decoding is the validation step: a truncated download or a non-image
        // payload fails here, before anything is installed.
        let original = image::open(&tmp_image)
            .map_err(|e| format!("The downloaded file is not a usable image: {e}"))?;
        let wallpaper = image_compose::compose_wallpaper(&original, width, height, fit);
        image_compose::save_jpeg(&wallpaper, &tmp_wallpaper)?;
        fs::rename(&tmp_image, &final_image)
            .map_err(|e| format!("Could not store the image: {e}"))?;
        fs::rename(&tmp_wallpaper, &final_wallpaper)
            .map_err(|e| format!("Could not store the wallpaper: {e}"))?;
        Ok(())
    })
    .await
    .map_err(|e| format!("Image task interrupted: {e}"))?;

    if composed.is_err() {
        let _ = fs::remove_file(&tmp_image);
        let _ = fs::remove_file(&tmp_wallpaper);
    }
    composed?;

    wallpaper::set_wallpaper(&final_wallpaper)?;

    let record = Applied {
        date: apod.date.clone(),
        title: apod.title.clone(),
        explanation: apod.explanation.clone(),
        copyright: apod.copyright.clone(),
        media_type: apod.media_type.clone(),
        video_url: if apod.is_video() {
            apod.url.clone()
        } else {
            None
        },
        source_url,
        image_file,
        wallpaper_file,
        fit,
        width,
        height,
        applied_on: today_string(),
    };
    state.lock().await.store.commit(record.clone())?;
    Ok(record)
}

/// Recomposes the wallpaper from the original already on disk, for a new fit
/// mode or a new screen size. No network access.
async fn reapply(
    state: &State,
    mut record: Applied,
    fit: FitMode,
    width: u32,
    height: u32,
) -> Result<(), String> {
    let (dir, original) = {
        let d = state.lock().await;
        (d.store.dir().to_path_buf(), d.store.image_path(&record))
    };

    let wallpaper_file = wallpaper_file_name(&record.date, fit, width, height);
    let tmp_wallpaper = dir.join(".incoming-wallpaper");
    let final_wallpaper = dir.join(&wallpaper_file);

    let tmp = tmp_wallpaper.clone();
    let target = final_wallpaper.clone();
    let composed = tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let image =
            image::open(&original).map_err(|e| format!("Could not read the stored image: {e}"))?;
        let wallpaper = image_compose::compose_wallpaper(&image, width, height, fit);
        image_compose::save_jpeg(&wallpaper, &tmp)?;
        fs::rename(&tmp, &target).map_err(|e| format!("Could not store the wallpaper: {e}"))
    })
    .await
    .map_err(|e| format!("Image task interrupted: {e}"))?;

    if composed.is_err() {
        let _ = fs::remove_file(&tmp_wallpaper);
    }
    composed?;

    wallpaper::set_wallpaper(&final_wallpaper)?;

    record.wallpaper_file = wallpaper_file;
    record.fit = fit;
    record.width = width;
    record.height = height;
    state.lock().await.store.commit(record)?;
    Ok(())
}

fn wallpaper_file_name(date: &str, fit: FitMode, width: u32, height: u32) -> String {
    // The screen size and fit mode are part of the name so a resolution or
    // monitor change forces a fresh composition, and because some desktops
    // (macOS in particular) cache the wallpaper by path and ignore a file
    // rewritten in place.
    let suffix = match fit {
        FitMode::BlurFill => "blur",
        FitMode::CropFill => "crop",
    };
    format!("wall-{date}-{suffix}-{width}x{height}.jpg")
}

fn image_extension(bytes: &[u8]) -> &'static str {
    match image::guess_format(bytes) {
        Ok(image::ImageFormat::Png) => "png",
        Ok(image::ImageFormat::Gif) => "gif",
        Ok(image::ImageFormat::WebP) => "webp",
        _ => "jpg",
    }
}

/// Informational notes shown next to an applied image.
fn note(settings: &Settings, a: &Applied, today: &str) -> Option<String> {
    let mut notes: Vec<String> = Vec::new();
    if settings.mode == Mode::Daily && a.date.as_str() < today {
        notes.push(format!(
            "Today's APOD is not published yet -- showing the most recent one ({}).",
            a.date
        ));
    }
    if a.media_type == "video" {
        notes.push(format!(
            "The APOD for {} is a video: its thumbnail is used as the wallpaper.",
            a.date
        ));
    }
    (!notes.is_empty()).then(|| notes.join(" "))
}

/// Marks the app as online and records the status note for the panel.
async fn finish(state: &State, message: Option<String>) {
    let mut d = state.lock().await;
    d.offline = false;
    d.status_message = message;
}

async fn offline(state: &State, e: ApiError) -> String {
    let message = e.to_string();
    let mut d = state.lock().await;
    d.offline = e.is_offline();
    let full = match d.store.applied() {
        Some(a) if d.offline => format!("Offline -- keeping the image from {}. {message}", a.date),
        _ => message,
    };
    d.status_message = Some(full.clone());
    full
}

async fn fail(state: &State, message: String) -> String {
    let mut d = state.lock().await;
    d.status_message = Some(message.clone());
    message
}

fn http_client() -> Result<reqwest::Client, String> {
    // Built per update and dropped afterwards: the app makes a couple of
    // requests a day, so keeping a connection pool and TLS state resident for
    // the process lifetime buys nothing.
    reqwest::Client::builder()
        .user_agent(concat!("apod-wallpaper/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| format!("Could not create the HTTP client: {e}"))
}

pub fn today_string() -> String {
    Local::now().format("%Y-%m-%d").to_string()
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
pub fn validate_apod_date(raw: &str) -> Result<NaiveDate, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("Pick a date first.".to_string());
    }
    let date = NaiveDate::parse_from_str(raw, "%Y-%m-%d")
        .map_err(|_| format!("Invalid date: \"{raw}\" (expected format: YYYY-MM-DD)."))?;
    if date < apod_start_date() {
        return Err(
            "The first APOD dates from 16 June 1995: pick a date from that day on.".to_string(),
        );
    }
    if date > Local::now().date_naive() {
        return Err("That date is in the future: pick today or an earlier date.".to_string());
    }
    Ok(date)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(date: &str, applied_on: &str) -> Applied {
        Applied {
            date: date.to_string(),
            title: "t".into(),
            explanation: "e".into(),
            copyright: None,
            media_type: "image".into(),
            video_url: None,
            source_url: "u".into(),
            image_file: "i.jpg".into(),
            wallpaper_file: "w.jpg".into(),
            fit: FitMode::BlurFill,
            width: 100,
            height: 100,
            applied_on: applied_on.to_string(),
        }
    }

    fn settings(mode: Mode, specific: &str) -> Settings {
        Settings {
            mode,
            specific_date: specific.to_string(),
            ..Settings::default()
        }
    }

    #[test]
    fn daily_is_on_target_only_for_todays_publication() {
        let s = settings(Mode::Daily, "");
        assert!(on_target(
            &s,
            &record("2026-07-28", "2026-07-28"),
            "2026-07-28"
        ));
        // Yesterday's picture applied today: today's is still awaited.
        assert!(!on_target(
            &s,
            &record("2026-07-27", "2026-07-28"),
            "2026-07-28"
        ));
    }

    #[test]
    fn random_is_on_target_for_any_image_drawn_today() {
        let s = settings(Mode::Random, "");
        assert!(on_target(
            &s,
            &record("1999-01-01", "2026-07-28"),
            "2026-07-28"
        ));
        assert!(!on_target(
            &s,
            &record("1999-01-01", "2026-07-27"),
            "2026-07-28"
        ));
    }

    #[test]
    fn specific_is_on_target_for_the_chosen_date_only() {
        let s = settings(Mode::Specific, "2001-02-03");
        assert!(on_target(
            &s,
            &record("2001-02-03", "2026-07-01"),
            "2026-07-28"
        ));
        assert!(!on_target(
            &s,
            &record("2001-02-04", "2026-07-28"),
            "2026-07-28"
        ));
    }

    #[test]
    fn wallpaper_name_carries_fit_and_screen_size() {
        assert_eq!(
            wallpaper_file_name("2026-07-28", FitMode::CropFill, 3456, 2234),
            "wall-2026-07-28-crop-3456x2234.jpg"
        );
    }

    #[test]
    fn rejects_dates_outside_the_archive() {
        assert!(validate_apod_date("1995-06-15").is_err());
        assert!(validate_apod_date("1995-06-16").is_ok());
        assert!(validate_apod_date("not a date").is_err());
        assert!(validate_apod_date("").is_err());
        assert!(validate_apod_date("2999-01-01").is_err());
    }
}
