use std::{hash::Hash, time::Duration};

mod inner;

use inner::RateLimiterInner;

use crate::context::RateLimitingError;

const MINUTE: Duration = Duration::from_secs(60);
const DAY: Duration = Duration::from_secs(60 * 60 * 24);

pub(super) struct RateLimiter<K> {
    minute: RateLimiterInner<K>,
    day: RateLimiterInner<K>,
}

impl<K> RateLimiter<K>
where
    K: PartialEq + Eq + Hash + Clone + Copy,
{
    pub fn new(
        minute_limit: usize,
        day_limit: usize,
        #[cfg(feature = "otel")] api_counter_fn: &dyn Fn(
            bencher_otel::IntervalKind,
        ) -> bencher_otel::ApiCounter,
        error: RateLimitingError,
    ) -> Self {
        let minute = RateLimiterInner::new(
            MINUTE,
            minute_limit,
            #[cfg(feature = "otel")]
            api_counter_fn(bencher_otel::IntervalKind::Minute),
            error.clone(),
        );

        let day = RateLimiterInner::new(
            DAY,
            day_limit,
            #[cfg(feature = "otel")]
            api_counter_fn(bencher_otel::IntervalKind::Day),
            error,
        );

        Self { minute, day }
    }

    pub fn check(&self, key: K) -> Result<(), dropshot::HttpError> {
        self.minute.check(key)?;
        self.day.check(key)?;

        Ok(())
    }
}
