//! Handing the composed image to the desktop.
//!
//! One function, two implementations, and the same contract on both: the image
//! handed over is already composed at the exact size of the screen, so nothing
//! here asks the desktop to scale, crop or tile it.
//!
//! Both are blocking -- an Apple event round trip on one side, a settings
//! write and its flush on the other -- so callers run this on the blocking
//! pool, never on a runtime worker. See `updater::apply_wallpaper`.

use std::path::Path;

/// Sets the image as the desktop wallpaper.
///
/// macOS sets the desktop picture through an Apple event to System Events,
/// applied to every desktop. The `wallpaper` crate wraps that AppleScript, and
/// `Info.plist` carries the `NSAppleEventsUsageDescription` without which the
/// system refuses the event outright on a signed build.
#[cfg(target_os = "macos")]
pub fn set_wallpaper(path: &Path) -> Result<(), String> {
    let path = path
        .to_str()
        .ok_or_else(|| "Invalid wallpaper path (UTF-8 expected).".to_string())?;

    // The image is already composed at the exact screen size, so no fit mode
    // is asked for here -- and macOS does not expose one through this crate
    // anyway.
    ::wallpaper::set_from_path(path).map_err(|e| format!("Could not set the wallpaper: {e}"))
}

/// GNOME keeps the desktop picture in GSettings rather than behind an API, so
/// setting it is writing three keys and flushing them.
///
/// GNOME caches the background by URI exactly as macOS caches it by path, and
/// the file name `updater::wallpaper_file_name` builds already carries the
/// date, the fit mode and the screen size -- so the cache is invalidated for
/// free, and neither platform needs a cache-busting step of its own.
#[cfg(target_os = "linux")]
pub fn set_wallpaper(path: &Path) -> Result<(), String> {
    use gio::prelude::*;

    let settings = background_settings()?;
    let uri = glib::filename_to_uri(path, None)
        .map_err(|e| format!("Could not build a file URI for the wallpaper: {e}"))?;

    // Both keys, so the picture does not follow the light/dark theme: it is
    // the same image either way, and leaving the dark one behind would swap
    // the wallpaper every time the theme changed.
    for key in ["picture-uri", "picture-uri-dark"] {
        settings
            .set_string(key, &uri)
            .map_err(|e| format!("Could not set {key}: {e}"))?;
    }

    // The composition already matches the screen exactly, so this only decides
    // what GNOME does on the occasions it disagrees about that size -- a
    // monitor change it noticed before we did, most often. Zoom fills without
    // distorting, which is the failure worth having.
    settings
        .set_string("picture-options", "zoom")
        .map_err(|e| format!("Could not set picture-options: {e}"))?;

    // GSettings writes back on an idle callback, and this process is quite
    // likely to go straight back to sleep for the rest of the day.
    gio::Settings::sync();
    Ok(())
}

/// Opens the background schema, or says why it cannot be opened.
///
/// `Settings::new` aborts the process when the schema is missing, which is
/// precisely what happens on a session that is not GNOME. Looking the schema
/// up first turns a crash into an error the panel can show.
#[cfg(target_os = "linux")]
fn background_settings() -> Result<gio::Settings, String> {
    const SCHEMA: &str = "org.gnome.desktop.background";

    gio::SettingsSchemaSource::default()
        .and_then(|source| source.lookup(SCHEMA, true))
        .ok_or_else(|| {
            format!(
                "The {SCHEMA} settings schema is not installed. \
                 This build sets the wallpaper through GNOME; \
                 it cannot do so on this desktop."
            )
        })?;

    Ok(gio::Settings::new(SCHEMA))
}
