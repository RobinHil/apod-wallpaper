//! The events the scheduler has to react to but cannot predict: the screen
//! layout changing, and the machine coming back from sleep.
//!
//! Without them the scheduler would have to wake up regularly just to compare
//! the screen size and the wall clock against what it last saw. With them it
//! sleeps from one day change to the next and still recomposes the wallpaper
//! seconds after a resolution change.
//!
//! Neither platform adds a library for this. macOS posts both as AppKit
//! notifications, and AppKit is already linked by Tauri's macOS backend; GNOME
//! posts both on D-Bus, and GLib -- which speaks D-Bus -- is already linked by
//! its Linux backend.

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
/// Failing to subscribe is not fatal: the scheduler still runs on its own
/// clock, it just stops noticing screen changes between day changes.
#[cfg(target_os = "macos")]
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

        // Both observers stay registered for the life of the process. Parking
        // them in a static would mean guarding two values that nothing ever
        // reads again.
        std::mem::forget(screens);
        std::mem::forget(wake);
    }
}

/// The GNOME half, with the same two events from two different buses.
///
/// Called from the main thread while the app starts, so the subscriptions land
/// on the GLib main context Tauri is already running; the callbacks then fire
/// on that loop and do nothing but release a permit, as the macOS blocks do.
///
/// Each subscription is independent: a compositor that does not answer costs
/// the screen-change wake-up and nothing else.
#[cfg(target_os = "linux")]
pub fn watch(wakeup: &Wakeup) {
    // Resolution or scale change, monitor plugged in or unplugged, displays
    // rearranged -- Mutter posts one signal for all of it, and the scheduler
    // settles for three seconds afterwards because a single change arrives as
    // a burst.
    subscribe(
        wakeup,
        gio::BusType::Session,
        "org.gnome.Mutter.DisplayConfig",
        "/org/gnome/Mutter/DisplayConfig",
        "org.gnome.Mutter.DisplayConfig",
        "MonitorsChanged",
        // No payload worth reading: that the layout changed is the whole
        // message, and `screen::size` is about to be asked what it changed to.
        |_| true,
    );

    // Waking from sleep, from logind rather than from the session: the pending
    // sleep is measured against a clock that does not advance while the
    // machine is suspended, so without this it would fire hours after
    // midnight.
    subscribe(
        wakeup,
        gio::BusType::System,
        "org.freedesktop.login1",
        "/org/freedesktop/login1",
        "org.freedesktop.login1.Manager",
        "PrepareForSleep",
        // The signal carries one boolean and fires twice per suspend: `true`
        // on the way down, `false` on the way back up. Only the second is a
        // reason to re-read the clock; acting on the first would run an update
        // into a machine that is about to stop executing it.
        |parameters| parameters.child_value(0).get::<bool>() == Some(false),
    );
}

/// Releases a permit on `wakeup` every time the named signal arrives and
/// `wanted` accepts its payload.
///
/// Reported rather than asserted, like the tray item: a bus that cannot be
/// reached leaves the scheduler on its own clock, which still sets the right
/// wallpaper every day.
#[cfg(target_os = "linux")]
fn subscribe(
    wakeup: &Wakeup,
    bus: gio::BusType,
    sender: &str,
    path: &str,
    interface: &str,
    signal: &str,
    wanted: fn(&glib::Variant) -> bool,
) {
    let connection = match gio::bus_get_sync(bus, gio::Cancellable::NONE) {
        Ok(connection) => connection,
        Err(e) => return eprintln!("no {signal} notifications ({bus:?} bus unreachable): {e}"),
    };

    let permit = wakeup.clone();
    connection.signal_subscribe(
        Some(sender),
        Some(interface),
        Some(signal),
        Some(path),
        None,
        gio::DBusSignalFlags::NONE,
        move |_, _, _, _, _, parameters| {
            if wanted(parameters) {
                permit.notify_one();
            }
        },
    );

    // The subscription lives on the connection, and the connection is dropped
    // at the end of this function unless it is kept. Same reasoning as the
    // macOS observers above: nothing ever reads it again, so parking it in a
    // static would mean guarding a value for the sake of guarding it.
    std::mem::forget(connection);
}
