//! The commands the panel invokes.
//!
//! Each waits for the operation to fully finish before answering: the frontend
//! blocks its UI meanwhile and shows any error it gets back. No work is ever
//! started in the background without its result being reported.
//!
//! Every one of them returns a whole `UiState`, which is the contract: the
//! panel adopts what it is handed and derives nothing of its own.

use crate::settings::{FitMode, Mode};
use crate::{SharedState, UiState, current_ui, refresh_ui, ui_state, updater};
use tauri::{AppHandle, Manager};

#[tauri::command]
pub async fn get_state(app: AppHandle) -> Result<UiState, String> {
    Ok(current_ui(&app).await)
}

/// Switches mode and applies it.
///
/// The new mode is persisted before the update runs and is *not* rolled back
/// if it fails: a mode is a standing preference, so a network outage should
/// leave the app aiming at what the user asked for and let the scheduler retry.
/// `set_specific_date` deliberately does the opposite -- see there.
#[tauri::command]
pub async fn set_mode(app: AppHandle, mode: Mode) -> Result<UiState, String> {
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
pub async fn set_specific_date(app: AppHandle, date: String) -> Result<UiState, String> {
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
pub async fn set_fit_mode(app: AppHandle, fit: FitMode) -> Result<UiState, String> {
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
pub async fn set_api_key(app: AppHandle, key: String) -> Result<UiState, String> {
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
pub async fn refresh_now(app: AppHandle) -> Result<UiState, String> {
    updater::update(&app, true).await?;
    Ok(current_ui(&app).await)
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}
