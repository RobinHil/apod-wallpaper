use crate::os_events::Wakeup;
use crate::updater::{self, Outcome};
use chrono::Local;
use std::time::Duration;
use tauri::AppHandle;

/// Longest single sleep.
///
/// The sleep is measured against a clock that does not advance while the
/// machine is suspended, so a night with the lid closed would push the day
/// change hours late. macOS and Windows say when they wake up (see
/// `os_events`), so there the sleep runs to the next day change and the
/// process is idle until then.
#[cfg(any(target_os = "macos", target_os = "windows"))]
const MAX_SLEEP: Duration = Duration::from_secs(25 * 3600);
/// On Linux nothing reports the resume, short of a D-Bus client for logind, so
/// the sleep is split and the wall clock re-read a few times a day. This does
/// not make the daily update any later: the remaining time is recomputed at
/// every wake, so the last stretch still ends exactly at the day change.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const MAX_SLEEP: Duration = Duration::from_secs(6 * 3600);
/// Interval used while waiting for today's APOD to be published. Deliberately
/// unhurried: this is not a failure, and polling the API every 15 minutes from
/// local midnight until publication would burn most of DEMO_KEY's 50 daily
/// requests just asking whether the picture is out yet. Half an hour late to a
/// daily wallpaper is invisible.
const PUBLICATION_RETRY: Duration = Duration::from_secs(30 * 60);
/// First retry delay after a failure; doubled on each further failure.
const BACKOFF_START: Duration = Duration::from_secs(10);
/// Ceiling for the retry delay.
const BACKOFF_MAX: Duration = Duration::from_secs(15 * 60);
/// Pause after an OS event before acting on it. Unplugging a monitor or
/// changing a resolution emits a burst of notifications, one per step of the
/// transition; waiting collapses them into a single recomposition and lets the
/// new mode settle before the screen is measured.
const SETTLE: Duration = Duration::from_secs(3);
/// Fallback when the next local midnight does not exist, which a DST jump can
/// arrange. An hour later the gap has been crossed.
const DST_GAP_RETRY: Duration = Duration::from_secs(3600);

/// The app's only background task.
///
/// It attempts an update, then sleeps until the next local day change or until
/// the OS reports something that invalidates the wallpaper -- a new screen
/// resolution, a machine waking up. Nothing else is armed: no polling of the
/// API, no periodic reapplication, no timer that exists only to check whether
/// anything has happened.
///
/// Retrying is therefore reserved for what actually failed. An attempt that
/// fails (offline, API outage) switches to an exponential backoff with jitter,
/// capped at 15 minutes, which stops as soon as one succeeds. A connection
/// attempt with no network returns locally in about a millisecond, so retrying
/// costs far less than keeping a connectivity-monitoring subsystem resident.
pub async fn run(app: AppHandle, wakeup: Wakeup) {
    let mut backoff = BACKOFF_START;
    loop {
        let wait = match updater::update(&app, false).await {
            Ok(Outcome::Satisfied) => {
                backoff = BACKOFF_START;
                until_next_day_change()
            }
            Ok(Outcome::AwaitingPublication) => {
                backoff = BACKOFF_START;
                PUBLICATION_RETRY
            }
            Err(_) => {
                let delay = with_jitter(backoff);
                backoff = (backoff * 2).min(BACKOFF_MAX);
                delay
            }
        };

        // Waits for the deadline, unless an OS event cuts the wait short.
        let interrupted = tokio::time::timeout(wait.min(MAX_SLEEP), wakeup.notified())
            .await
            .is_ok();
        if interrupted {
            tokio::time::sleep(SETTLE).await;
        }
    }
}

/// Time left until just after the next local midnight. Computed from the wall
/// clock every time, never accumulated.
fn until_next_day_change() -> Duration {
    let now = Local::now();
    now.date_naive()
        .succ_opt()
        // A few seconds past midnight, so the local date has certainly ticked
        // over by the time we look at it.
        .and_then(|tomorrow| tomorrow.and_hms_opt(0, 0, 5))
        // `earliest()` resolves the ambiguity when a DST change makes local
        // midnight happen twice; `None` when it does not exist at all.
        .and_then(|naive| naive.and_local_timezone(Local).earliest())
        .and_then(|next| (next - now).to_std().ok())
        .unwrap_or(DST_GAP_RETRY)
}

/// Spreads retries by +/-20% so several machines that lost the same network do
/// not all come back at the same instant.
fn with_jitter(delay: Duration) -> Duration {
    use rand::RngExt;
    delay.mul_f64(rand::rng().random_range(0.8..1.2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn day_change_is_always_within_a_day() {
        let wait = until_next_day_change();
        assert!(wait <= Duration::from_secs(24 * 3600 + 5));
    }

    #[test]
    fn the_sleep_cap_never_delays_the_day_change() {
        // Whether the sleep is split or not, the last stretch is the exact
        // remainder to just past midnight, so the cap only ever bounds how
        // long a suspended machine takes to notice. This guards the day the
        // cap is lowered below the longest possible wait without splitting the
        // update off the day change.
        let longest_possible_wait = Duration::from_secs(24 * 3600 + 5);
        assert!(
            MAX_SLEEP >= longest_possible_wait
                || cfg!(not(any(target_os = "macos", target_os = "windows"))),
            "a capped sleep is only acceptable where the OS reports no resume"
        );
    }

    #[test]
    fn jitter_stays_within_twenty_percent() {
        let base = Duration::from_secs(100);
        for _ in 0..100 {
            let d = with_jitter(base);
            assert!(d >= Duration::from_secs(80) && d <= Duration::from_secs(120));
        }
    }

    #[test]
    fn backoff_doubles_up_to_the_cap() {
        let mut backoff = BACKOFF_START;
        let mut seen = vec![];
        for _ in 0..12 {
            seen.push(backoff);
            backoff = (backoff * 2).min(BACKOFF_MAX);
        }
        assert_eq!(seen[0], Duration::from_secs(10));
        assert_eq!(seen[1], Duration::from_secs(20));
        assert_eq!(seen[2], Duration::from_secs(40));
        assert_eq!(*seen.last().unwrap(), BACKOFF_MAX);
    }
}
