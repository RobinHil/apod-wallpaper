//! Measuring the display the wallpaper is composed for.
//!
//! The main display, the one carrying the menu bar or the top bar. The others
//! get the same image, which is a documented limitation in the README.
//!
//! `None` is a meaningful answer and not a failure: it means the desktop
//! reports no main display at all, the lid closed with no external screen
//! being the usual way. Callers must not treat it as a resolution change --
//! recomposing for a guessed size would replace a correct wallpaper with a
//! wrong one -- and `updater::run` falls back to the size already recorded.

use tauri::AppHandle;

/// Physical resolution of the main display, clamped to the smallest wallpaper
/// `image_compose` will produce.
///
/// The clamp and the composition share [`crate::image_compose::MIN_SCREEN`]
/// because they must agree: a screen measured below the floor but composed at
/// it would be recorded at one size and drawn at another, and every later
/// comparison would see a resolution change that never happened.
pub fn size(app: &AppHandle) -> Option<(u32, u32)> {
    let (width, height) = measure(app)?;
    let (min_w, min_h) = crate::image_compose::MIN_SCREEN;
    Some((width.max(min_w), height.max(min_h)))
}

/// Tauri reports the backing size directly on macOS, Retina scaling included.
#[cfg(target_os = "macos")]
fn measure(app: &AppHandle) -> Option<(u32, u32)> {
    let monitor = app.primary_monitor().ok()??;
    let size = monitor.size();
    Some((size.width, size.height))
}

/// On GNOME the compositor is asked, not the toolkit.
///
/// `app.primary_monitor()` would be the portable answer, and it is the wrong
/// one here. Under Wayland with fractional scaling GTK 3 reports the *logical*
/// size: a 1920x1200 panel at 125% comes back as 1536x960. Composing from that
/// would hand GNOME an image it then has to enlarge, which is exactly the
/// blurry wallpaper this application exists to avoid.
///
/// Mutter knows the real mode, so Mutter is asked. The reply is a nest of
/// tuples; the shape is documented at
/// <https://gitlab.gnome.org/GNOME/mutter/-/blob/main/data/dbus-interfaces/org.gnome.Mutter.DisplayConfig.xml>
/// and reproduced in the comments below, because reading it back off the
/// indices alone is unpleasant.
#[cfg(target_os = "linux")]
fn measure(_app: &AppHandle) -> Option<(u32, u32)> {
    let connection = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE).ok()?;
    let reply = connection
        .call_sync(
            Some("org.gnome.Mutter.DisplayConfig"),
            "/org/gnome/Mutter/DisplayConfig",
            "org.gnome.Mutter.DisplayConfig",
            "GetCurrentState",
            None,
            None,
            gio::DBusCallFlags::NONE,
            // Generous, and still bounded: this runs before every composition,
            // and a compositor that has stopped answering must not park the
            // update for ever.
            2_000,
            gio::Cancellable::NONE,
        )
        .ok()?;

    // (serial, monitors, logical_monitors, properties)
    let monitors = reply.child_value(1);
    let logical = reply.child_value(2);

    // A logical monitor is (x, y, scale, transform, primary, [monitor specs],
    // properties). The primary one names the physical monitor we want; with
    // none flagged -- which happens on some multi-head layouts -- the first is
    // as good an answer as any, and matches what GNOME itself puts first.
    let chosen = logical
        .iter()
        .find(|monitor| monitor.child_value(4).get::<bool>().unwrap_or(false))
        .or_else(|| logical.iter().next())?;

    // Each spec is (connector, vendor, product, serial); the connector is what
    // ties it back to the monitors array.
    let connector = chosen
        .child_value(5)
        .iter()
        .next()?
        .child_value(0)
        .get::<String>()?;

    // A monitor is ((connector, vendor, product, serial), [modes], properties)
    // and a mode is (id, width, height, refresh, preferred_scale,
    // [supported_scales], properties), with "is-current" marking the one in
    // use. That mode's width and height are the real pixels.
    let monitor = monitors
        .iter()
        .find(|m| m.child_value(0).child_value(0).get::<String>().as_deref() == Some(&connector))?;

    let mode = monitor.child_value(1).iter().find(|mode| {
        glib::VariantDict::new(Some(&mode.child_value(6)))
            .lookup::<bool>("is-current")
            .ok()
            .flatten()
            .unwrap_or(false)
    })?;

    let width = mode.child_value(1).get::<i32>()?;
    let height = mode.child_value(2).get::<i32>()?;
    (width > 0 && height > 0).then_some((width as u32, height as u32))
}
