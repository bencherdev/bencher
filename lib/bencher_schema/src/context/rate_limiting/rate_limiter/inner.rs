use std::{
    collections::VecDeque,
    hash::Hash,
    time::{Duration, SystemTime},
};

use dashmap::DashMap;
use dropshot::HttpError;

use crate::{context::RateLimitingError, error::too_many_requests};

pub(super) struct RateLimiterInner<K> {
    event_map: DashMap<K, VecDeque<SystemTime>>,
    window: Duration,
    limit: usize,
    #[cfg(feature = "otel")]
    api_counter_max: bencher_otel::ApiCounter,
    error: RateLimitingError,
}

pub struct RateLimiterConfig {
    pub window: Duration,
    pub limit: usize,
    #[cfg(feature = "otel")]
    pub api_counter_max: bencher_otel::ApiCounter,
    pub error: RateLimitingError,
}

impl<K> From<RateLimiterConfig> for RateLimiterInner<K>
where
    K: PartialEq + Eq + Hash,
{
    fn from(config: RateLimiterConfig) -> Self {
        let RateLimiterConfig {
            window,
            limit,
            #[cfg(feature = "otel")]
            api_counter_max,
            error,
        } = config;
        Self {
            event_map: DashMap::new(),
            window,
            limit,
            #[cfg(feature = "otel")]
            api_counter_max,
            error,
        }
    }
}

impl<K> RateLimiterInner<K>
where
    K: PartialEq + Eq + Hash,
{
    pub fn check(&self, key: K) -> Result<(), HttpError> {
        let now = SystemTime::now();
        let cutoff = now - self.window;

        // Clean up old times for all keys
        self.event_map.retain(|_, times| {
            // Since times are in ascending order, remove from front until we hit a recent one
            while times.front().is_some_and(|&time| time < cutoff) {
                times.pop_front();
            }
            !times.is_empty()
        });

        let mut entry = self
            .event_map
            .entry(key)
            .or_insert_with(|| VecDeque::with_capacity(self.limit));

        // Check if the limit has been exceeded
        if entry.len() < self.limit {
            // Record the new time for the key
            entry.push_back(now);

            Ok(())
        } else {
            // Remove the oldest time and add the new one
            entry.pop_front();
            entry.push_back(now);

            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(self.api_counter_max);

            Err(too_many_requests(self.error.clone()))
        }
    }
}
