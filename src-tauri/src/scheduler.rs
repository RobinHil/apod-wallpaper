use crate::updater::{self, Outcome};
use chrono::Local;
use std::time::Duration;
use tauri::AppHandle;

/// Longest single sleep. Each wake recomputes the remaining time from the wall
/// clock, so a suspended machine, a clock adjustment or a time-zone change
/// corrects itself within one chunk. That costs two wakes an hour, each a
/// handful of string comparisons, and saves three platform-specific power and
/// clock notification backends.
const MAX_SLEEP: Duration = Duration::from_secs(30 * 60);
/// First retry delay after a failure; doubled on each further failure.
const BACKOFF_START: Duration = Duration::from_secs(10);
/// Ceiling for the retry delay.
const BACKOFF_MAX: Duration = Duration::from_secs(15 * 60);

/// The app's only background task.
///
/// It attempts an update, then sleeps until the next local day change. Nothing
/// is armed while everything is up to date, beyond that single sleep: there is
/// no polling of the API and no periodic wallpaper reapplication.
///
/// A failed attempt (offline, API outage) or an APOD that is not published yet
/// switches to an exponential backoff with jitter, capped at 15 minutes, which
/// stops as soon as an attempt succeeds. A failed connection with no network
/// returns locally in about a millisecond, so retrying costs far less than
/// keeping a connectivity-monitoring subsystem resident.
pub async fn run(app: AppHandle) {
    let mut backoff = BACKOFF_START;
    loop {
        let wait = match updater::update(&app, false).await {
            Ok(Outcome::Satisfied) => {
                backoff = BACKOFF_START;
                until_next_day_change()
            }
            Ok(Outcome::Retry) | Err(_) => {
                let delay = with_jitter(backoff);
                backoff = (backoff * 2).min(BACKOFF_MAX);
                delay
            }
        };
        tokio::time::sleep(wait.min(MAX_SLEEP)).await;
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
        // midnight happen twice; `None` when it does not exist at all, in
        // which case the caller's cap takes over.
        .and_then(|naive| naive.and_local_timezone(Local).earliest())
        .and_then(|next| (next - now).to_std().ok())
        .unwrap_or(MAX_SLEEP)
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
