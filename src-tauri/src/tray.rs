//! The tray item: the menu bar on macOS, the top bar on GNOME.
//!
//! Read-only. It shows the current image's title and credits, opens the panel
//! and quits. Every setting lives in the panel instead.
//!
//! It is a convenience and never the only way in. Nothing depends on it: the
//! panel carries every setting, the manual refresh and the quit button;
//! launching the application again brings that panel up; and an item that
//! fails to build is logged and stepped over rather than being a startup
//! error. That last point is defensive on macOS and routine on GNOME, whose
//! shell shows no status icons at all without an extension.

use crate::UiState;
use crate::panel::show_panel;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Wry};

/// Handles to the tray entries whose text changes over time.
pub struct Handles {
    title: MenuItem<Wry>,
    info: MenuItem<Wry>,
}

pub fn build(app: &tauri::App) -> tauri::Result<()> {
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

    app.manage(Handles { title, info });

    // Reported rather than asserted: the caller treats a tray item it
    // could not build as a missing convenience and starts anyway, which a
    // panic here would turn back into a fatal error.
    let icon = app
        .default_window_icon()
        .ok_or_else(|| {
            tauri::Error::Io(std::io::Error::other(
                "the bundle carries no default icon to put in the tray",
            ))
        })?
        .clone();

    TrayIconBuilder::with_id("apod-tray")
        .icon(icon)
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

/// Rewrites the two labels from the state the backend just published.
///
/// Silent when there is no tray: `refresh_ui` calls this on every push, and on
/// a GNOME without the AppIndicator extension there is simply nothing to
/// write to.
pub fn update_labels(app: &AppHandle, ui: &UiState) {
    let Some(tray) = app.try_state::<Handles>() else {
        return;
    };

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

/// Shortens a label to fit a menu, counting characters and not bytes: a title
/// cut mid-character would reach the tray as broken text.
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(3)).collect();
    out.push_str("...");
    out
}

#[cfg(test)]
mod tests {
    use super::truncate;

    #[test]
    fn truncate_keeps_short_titles_and_bounds_long_ones() {
        assert_eq!(truncate("Andromeda", 20), "Andromeda");
        assert_eq!(truncate("Andromeda", 6), "And...");
        // Counted in characters, not bytes: a title cut mid-character would
        // reach the tray as broken text.
        assert_eq!(
            truncate("\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}", 4),
            "\u{e9}..."
        );
    }
}
