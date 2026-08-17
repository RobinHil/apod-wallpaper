//! Handing the composed image to the desktop.
//!
//! macOS sets the desktop picture through an Apple event to System Events,
//! applied to every desktop. The `wallpaper` crate wraps that AppleScript, and
//! `Info.plist` carries the `NSAppleEventsUsageDescription` without which the
//! system refuses the event outright on a signed build.

use std::path::Path;

/// Sets the image as the desktop wallpaper.
///
/// Thoroughly blocking -- an Apple event round trip, seconds of it when the
/// desktop is busy -- so callers run it on the blocking pool, never on a
/// runtime worker.
pub fn set_wallpaper(path: &Path) -> Result<(), String> {
    let path = path
        .to_str()
        .ok_or_else(|| "Invalid wallpaper path (UTF-8 expected).".to_string())?;

    // The image is already composed at the exact screen size, so no fit mode
    // is asked for here -- and macOS does not expose one through this crate
    // anyway.
    ::wallpaper::set_from_path(path).map_err(|e| format!("Could not set the wallpaper: {e}"))
}
