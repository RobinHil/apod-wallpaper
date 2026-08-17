//! The events the scheduler has to react to but cannot predict: the screen
//! layout changing, and the machine coming back from sleep.
//!
//! Without them the scheduler would have to wake up regularly just to compare
//! the screen size and the wall clock against what it last saw. With them it
//! sleeps from one day change to the next and still recomposes the wallpaper
//! seconds after a resolution change.
//!
//! Both notifications come from AppKit, which Tauri's macOS backend already
//! links, so none of this adds a crate to the build.

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
