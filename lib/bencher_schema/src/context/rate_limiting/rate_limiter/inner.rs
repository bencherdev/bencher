use std::{
    collections::VecDeque,
    hash::Hash,
    time::{Duration, SystemTime},
};

use dashmap::DashMap;
use dropshot::HttpError;

use crate::{context::RateLimitingError, error::too_many_requests};

// Set the default capacity to `1` to minimize the overhead of traffic from disparate sources by default.
// If an IP is being abusive, we will have to reallocate quite a few times before they hit their limit.
// However, this is a tradeoff to reduce the memory usage on the happy path.
const DEFAULT_CAPACITY: usize = 1;

pub(super) struct RateLimiterInner<K> {
    window: Duration,
    limit: usize,
    event_map: DashMap<K, VecDeque<SystemTime>>,
    #[cfg(feature = "otel")]
    api_counter_max: bencher_otel::ApiCounter,
    error: RateLimitingError,
}

impl<K> RateLimiterInner<K>
where
    K: PartialEq + Eq + Hash,
{
    pub fn new(
        window: Duration,
        limit: usize,
        #[cfg(feature = "otel")] api_counter_max: bencher_otel::ApiCounter,
        error: RateLimitingError,
    ) -> Self {
        Self {
            window,
            limit,
            event_map: DashMap::new(),
            #[cfg(feature = "otel")]
            api_counter_max,
            error,
        }
    }

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
            .or_insert_with(|| VecDeque::with_capacity(DEFAULT_CAPACITY));

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
