//! The events the scheduler has to react to but cannot predict: the primary
//! monitor changing size, and the machine coming back from sleep.
//!
//! Without them the scheduler would have to wake up regularly just to compare
//! the screen size and the wall clock against what it last saw. With them it
//! sleeps from one day change to the next and still recomposes the wallpaper
//! seconds after a resolution change.
//!
//! Each platform is served by the toolkit its Tauri backend already links, so
//! none of this adds a crate to the build: AppKit on macOS, GDK on Linux,
//! Win32 on Windows.

use std::sync::Arc;
use tokio::sync::Notify;

/// Signal from the OS observers to the scheduler.
///
/// `Notify` keeps one permit when nobody is waiting, so an event that lands
/// while an update is already running is not lost: the following wait returns
/// immediately instead of sleeping through it.
pub type Wakeup = Arc<Notify>;

/// Subscribes to the OS notifications. Called once, from the main thread,
/// while the app starts.
///
/// Failing to subscribe is not fatal anywhere: the scheduler still runs on its
/// own clock, it just stops noticing screen changes between day changes.
pub fn watch(wakeup: &Wakeup) {
    platform::watch(wakeup);
}

// ---------------------------------------------------------------------------
// macOS
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
mod platform {
    use super::Wakeup;

    pub fn watch(wakeup: &Wakeup) {
        use block2::RcBlock;
        use objc2_app_kit::{
            NSApplicationDidChangeScreenParametersNotification, NSWorkspace,
            NSWorkspaceDidWakeNotification,
        };
        use objc2_foundation::{NSNotification, NSNotificationCenter};
        use std::ptr::NonNull;

        let signal = wakeup.clone();
        let block = RcBlock::new(move |_: NonNull<NSNotification>| signal.notify_one());

        unsafe {
            // Resolution or scale change, monitor plugged in or unplugged,
            // displays rearranged.
            let screens = NSNotificationCenter::defaultCenter()
                .addObserverForName_object_queue_usingBlock(
                    Some(NSApplicationDidChangeScreenParametersNotification),
                    None,
                    // No queue: the block runs on the thread posting the
                    // notification, and all it does is release a permit.
                    None,
                    &block,
                );

            // Waking from sleep. The pending sleep is measured against a clock
            // that does not advance while the machine is suspended, so without
            // this it would fire hours after midnight.
            let wake = NSWorkspace::sharedWorkspace()
                .notificationCenter()
                .addObserverForName_object_queue_usingBlock(
                    Some(NSWorkspaceDidWakeNotification),
                    None,
                    None,
                    &block,
                );

            // Both observers stay registered for the life of the process.
            // Parking them in a static would mean guarding two values that
            // nothing ever reads again.
            std::mem::forget(screens);
            std::mem::forget(wake);
        }
    }
}

// ---------------------------------------------------------------------------
// Linux
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
mod platform {
    use super::Wakeup;

    /// GDK sits on top of both X11 and Wayland, so one subscription covers
    /// every session type the `wallpaper` crate supports.
    ///
    /// Resume from suspend is not covered: it would mean talking to logind
    /// over D-Bus, a dependency and a client for one signal. The scheduler
    /// keeps a slow re-check on this platform instead.
    pub fn watch(wakeup: &Wakeup) {
        // No `gdk::prelude` import: as of gtk-rs 0.18 `Screen::default` and
        // the two `connect_*` methods below are inherent on `Screen` rather
        // than coming from `ScreenExt`, so importing the prelude is an unused
        // import and nothing more.

        // GTK is initialised by Tauri before the setup hook runs; if it is
        // not, there is no display to watch anyway.
        let Some(screen) = gdk::Screen::default() else {
            return;
        };

        let resized = wakeup.clone();
        screen.connect_size_changed(move |_| resized.notify_one());

        let rearranged = wakeup.clone();
        screen.connect_monitors_changed(move |_| rearranged.notify_one());
    }
}

// ---------------------------------------------------------------------------
// Windows
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
mod platform {
    use super::Wakeup;
    use std::sync::OnceLock;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    // `PBT_APMRESUMEAUTOMATIC` lives with the window messages rather than in
    // `System::Power`: it is a `WM_POWERBROADCAST` payload, not a power API.
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, RegisterClassW,
        TranslateMessage, CW_USEDEFAULT, MSG, PBT_APMRESUMEAUTOMATIC, WM_DISPLAYCHANGE,
        WM_POWERBROADCAST, WNDCLASSW, WS_OVERLAPPED,
    };

    /// The window procedure runs on its own thread and cannot capture, so the
    /// signal is reached through a static. There is only ever one watcher.
    static SIGNAL: OnceLock<Wakeup> = OnceLock::new();

    /// Win32 broadcasts `WM_DISPLAYCHANGE` to top-level windows only -- a
    /// message-only window would never see it. So this creates a real window
    /// and simply never shows it: it has no visible presence, no taskbar
    /// entry, and its thread spends its life blocked in `GetMessageW`.
    pub fn watch(wakeup: &Wakeup) {
        if SIGNAL.set(wakeup.clone()).is_err() {
            return;
        }
        std::thread::Builder::new()
            .name("apod-display-watch".into())
            .spawn(pump_messages)
            .ok();
    }

    fn pump_messages() {
        // UTF-16, null terminated, as every `W` entry point expects.
        let class: Vec<u16> = "ApodWallpaperDisplayWatcher\0".encode_utf16().collect();

        unsafe {
            let instance = GetModuleHandleW(std::ptr::null());
            let mut descriptor: WNDCLASSW = std::mem::zeroed();
            descriptor.lpfnWndProc = Some(window_proc);
            descriptor.hInstance = instance;
            descriptor.lpszClassName = class.as_ptr();
            if RegisterClassW(&descriptor) == 0 {
                return;
            }

            let window = CreateWindowExW(
                0,
                class.as_ptr(),
                class.as_ptr(),
                WS_OVERLAPPED,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                instance,
                std::ptr::null(),
            );
            if window.is_null() {
                return;
            }

            // Blocks until a message arrives: no timer, no polling.
            let mut message: MSG = std::mem::zeroed();
            while GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }

    unsafe extern "system" fn window_proc(
        window: HWND,
        message: u32,
        w: WPARAM,
        l: LPARAM,
    ) -> LRESULT {
        let interesting = message == WM_DISPLAYCHANGE
            || (message == WM_POWERBROADCAST && w == PBT_APMRESUMEAUTOMATIC as WPARAM);
        if interesting {
            if let Some(signal) = SIGNAL.get() {
                signal.notify_one();
            }
        }
        unsafe { DefWindowProcW(window, message, w, l) }
    }
}
