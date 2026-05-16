mod bandwidth;
mod rate_limiter;
pub mod snapshot;

pub use bandwidth::BandwidthLimiter;
pub use rate_limiter::{Interval, RateLimiter, RateLimits};

#[expect(clippy::integer_division)]
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
