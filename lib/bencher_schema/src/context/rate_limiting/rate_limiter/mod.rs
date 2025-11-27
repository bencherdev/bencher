use std::hash::Hash;

mod inner;

pub use inner::RateLimiterConfig;
use inner::RateLimiterInner;

pub(super) struct RateLimiter<K> {
    minute: RateLimiterInner<K>,
    day: RateLimiterInner<K>,
}

impl<K> RateLimiter<K>
where
    K: PartialEq + Eq + Hash + Clone + Copy,
{
    pub fn new(minute_config: RateLimiterConfig, day_config: RateLimiterConfig) -> Self {
        Self {
            minute: minute_config.into(),
            day: day_config.into(),
        }
    }

    pub fn check(&self, key: K) -> Result<(), dropshot::HttpError> {
        self.minute.check(key)?;
        self.day.check(key)?;
        Ok(())
    }
}
