//! The settings panel window.
//!
//! Built on demand and destroyed when closed, so nothing is lost when it goes:
//! every setting is persisted by the backend as it is changed, and the window
//! is rebuilt from the state the backend hands back.
//!
//! What closing it does *not* do is give the memory back, and that is worth
//! stating plainly rather than assuming. Measured on GNOME: 28.6 MB of shared
//! memory for a process that has never opened the panel, against 154 MB once
//! it has been opened and closed again -- a WebKitNetworkProcess outlives the
//! window it was started for, and the webview's share of the main process is
//! not returned either. Destroying the window frees what Tauri owns; the rest
//! belongs to WebKitGTK, which keeps it for the life of the process.
//!
//! So the panel is cheap to leave open and expensive to have opened at all.
//! Which is exactly why nothing in this application requires opening it: the
//! scheduler, the wallpaper and the tray labels all work with no window ever
//! built, and a user who never opens the panel pays none of this.

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

/// Label of the settings panel window, matching `capabilities/default.json`.
pub const PANEL: &str = "main";

/// Opens the settings panel, creating the window if it does not exist.
pub fn show_panel(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(PANEL) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        return;
    }

    let built = WebviewWindowBuilder::new(app, PANEL, WebviewUrl::default())
        .title("APOD Wallpaper")
        .inner_size(440.0, 640.0)
        .resizable(false)
        .maximizable(false)
        .center()
        // Hidden from the app switcher on macOS: the menu bar is the way back
        // to a background utility, and the activation policy set at startup
        // keeps it out of the Dock anyway.
        //
        // Not on GNOME, where the same call would be a trap. There may be no
        // tray icon at all there, so a panel that is missing from the overview
        // and from alt-tab could be covered by another window with no way back
        // to it short of launching the application again. While it is open it
        // is an ordinary window, and the shell should treat it as one.
        .skip_taskbar(cfg!(target_os = "macos"))
        .build();

    match built {
        Ok(window) => {
            let _ = window.set_focus();
        }
        Err(e) => eprintln!("could not open the settings panel: {e}"),
    }
}
