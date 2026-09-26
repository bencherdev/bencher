use std::time::{Duration, SystemTime};

mod bandwidth;
mod rate_limiter;
pub mod snapshot;

pub use bandwidth::BandwidthLimiter;
pub use rate_limiter::{DAY, HOUR, Interval, MINUTE, RateLimiter, RateLimits};

use snapshot::EpochBucket;

const BUCKETS_PER_WINDOW: u64 = 12;

/// Buckets events by a twelfth of the window, so an event counts for at least
/// the window and at most one bucket longer.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Buckets {
    window_secs: u64,
    bucket_secs: u64,
}

impl Buckets {
    pub(crate) fn new(window: Duration) -> Self {
        let window_secs = window.as_secs();
        Self {
            window_secs,
            bucket_secs: window_secs.saturating_div(BUCKETS_PER_WINDOW).max(1),
        }
    }

    pub(crate) fn now_bucket(self, now: SystemTime) -> EpochBucket {
        epoch_bucket(epoch_secs(now), self.bucket_secs)
    }

    /// The oldest bucket that still overlaps the window ending at `now`.
    pub(crate) fn cutoff_bucket(self, now: SystemTime) -> EpochBucket {
        epoch_bucket(
            epoch_secs(now).saturating_sub(self.window_secs),
            self.bucket_secs,
        )
    }
}

fn epoch_secs(time: SystemTime) -> u64 {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[expect(
    clippy::integer_division,
    reason = "intentional truncation to compute bucket index"
)]
pub fn epoch_bucket(epoch_secs: u64, bucket_secs: u64) -> u64 {
    debug_assert!(bucket_secs > 0, "bucket_secs must be non-zero");
    epoch_secs / bucket_secs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_bucket_basic() {
        assert_eq!(epoch_bucket(120, 60), 2);
        assert_eq!(epoch_bucket(3600, 3600), 1);
        assert_eq!(epoch_bucket(86400, 86400), 1);
    }

    #[test]
    fn epoch_bucket_boundary() {
        assert_eq!(epoch_bucket(59, 60), 0);
        assert_eq!(epoch_bucket(60, 60), 1);
        assert_eq!(epoch_bucket(0, 60), 0);
    }
}
