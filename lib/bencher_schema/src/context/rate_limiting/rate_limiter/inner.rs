use std::{
    collections::{HashMap, VecDeque},
    hash::Hash,
    time::{Duration, SystemTime},
};

use dashmap::DashMap;
use dropshot::HttpError;

use crate::context::{
    RateLimitingError,
    rate_limiting::snapshot::{EpochMinutes, RateLimiterInnerSnapshot},
};
use crate::error::too_many_requests;

use super::super::epoch_minute;

const DEFAULT_CAPACITY: usize = 1;

pub(super) struct RateLimiterInner<K> {
    window: Duration,
    limit: usize,
    event_map: DashMap<K, BucketedEvents>,
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

    fn now_and_cutoff(&self) -> (u64, u64) {
        let now_secs = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let now_minute = epoch_minute(now_secs);
        let cutoff_minute = epoch_minute(now_secs.saturating_sub(self.window.as_secs()));
        (now_minute, cutoff_minute)
    }

    pub fn snapshot(&self) -> RateLimiterInnerSnapshot<K>
    where
        K: Clone,
    {
        let (_, cutoff_minute) = self.now_and_cutoff();
        let mut events = HashMap::new();
        for entry in &self.event_map {
            let buckets: Vec<(EpochMinutes, u32)> = entry
                .value()
                .buckets
                .iter()
                .filter(|(minute, _)| *minute >= cutoff_minute)
                .copied()
                .collect();
            if !buckets.is_empty() {
                events.insert(entry.key().clone(), buckets);
            }
        }
        RateLimiterInnerSnapshot { events }
    }

    pub fn restore(&self, snapshot: RateLimiterInnerSnapshot<K>) {
        let (_, cutoff_minute) = self.now_and_cutoff();
        for (key, buckets) in snapshot.events {
            let filtered: VecDeque<(u64, u32)> = buckets
                .into_iter()
                .filter(|(minute, _)| *minute >= cutoff_minute)
                .collect();
            let total: usize = filtered.iter().map(|(_, c)| *c as usize).sum();
            if total > 0 {
                self.event_map.insert(
                    key,
                    BucketedEvents {
                        buckets: filtered,
                        total,
                    },
                );
            }
        }
    }

    pub fn prune(&self) {
        let (_, cutoff_minute) = self.now_and_cutoff();
        self.event_map.retain(|_, events| {
            events.prune(cutoff_minute);
            events.total > 0
        });
    }

    pub fn check(&self, key: K) -> Result<(), HttpError> {
        let (now_minute, cutoff_minute) = self.now_and_cutoff();

        let mut entry = self
            .event_map
            .entry(key)
            .or_insert_with(|| BucketedEvents::with_capacity(DEFAULT_CAPACITY));
        entry.prune(cutoff_minute);

        if entry.total < self.limit {
            entry.record(now_minute);
            Ok(())
        } else {
            entry.evict_oldest();
            entry.record(now_minute);

            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(self.api_counter_max);

            Err(too_many_requests(self.error.clone()))
        }
    }
}

struct BucketedEvents {
    buckets: VecDeque<(u64, u32)>,
    total: usize,
}

impl BucketedEvents {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            buckets: VecDeque::with_capacity(capacity),
            total: 0,
        }
    }

    fn prune(&mut self, cutoff_minute: u64) {
        while self
            .buckets
            .front()
            .is_some_and(|(minute, _)| *minute < cutoff_minute)
        {
            if let Some((_, count)) = self.buckets.pop_front() {
                self.total -= count as usize;
            }
        }
    }

    fn record(&mut self, now_minute: u64) {
        if let Some((minute, count)) = self.buckets.back_mut()
            && *minute == now_minute
        {
            *count += 1;
            self.total += 1;
            return;
        }
        self.buckets.push_back((now_minute, 1));
        self.total += 1;
    }

    fn evict_oldest(&mut self) {
        if let Some((_, count)) = self.buckets.front_mut() {
            if *count > 1 {
                *count -= 1;
            } else {
                self.buckets.pop_front();
            }
            self.total -= 1;
        }
    }
}
