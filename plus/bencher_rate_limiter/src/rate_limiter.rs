use std::{
    collections::{HashMap, VecDeque},
    hash::Hash,
    time::{Duration, SystemTime},
};

use dashmap::DashMap;

use crate::Buckets;
use crate::snapshot::{EpochBucket, RateLimiterSnapshot, WindowSnapshot};

pub const MINUTE: Duration = Duration::from_mins(1);
pub const HOUR: Duration = Duration::from_hours(1);
pub const DAY: Duration = Duration::from_hours(24);

const DEFAULT_CAPACITY: usize = 1;

pub struct RateLimiter<K> {
    minute: Window<K>,
    hour: Window<K>,
    day: Window<K>,
}

#[derive(Debug, Clone, Copy)]
pub struct RateLimits {
    pub minute: usize,
    pub hour: usize,
    pub day: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interval {
    Minute,
    Hour,
    Day,
}

impl std::fmt::Display for Interval {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Minute => write!(f, "minute"),
            Self::Hour => write!(f, "hour"),
            Self::Day => write!(f, "day"),
        }
    }
}

impl<K> RateLimiter<K>
where
    K: PartialEq + Eq + Hash + Clone + Copy,
{
    pub fn new(limits: RateLimits) -> Self {
        let RateLimits { minute, hour, day } = limits;
        Self {
            minute: Window::new(MINUTE, minute),
            hour: Window::new(HOUR, hour),
            day: Window::new(DAY, day),
        }
    }

    pub fn check(&self, key: K) -> Option<Interval> {
        let minute = self.minute.check(key);
        let hour = self.hour.check(key);
        let day = self.day.check(key);
        if !minute {
            Some(Interval::Minute)
        } else if !hour {
            Some(Interval::Hour)
        } else if !day {
            Some(Interval::Day)
        } else {
            None
        }
    }

    /// Compact all 3 windows, dropping every key whose events have all aged out.
    ///
    /// Returns the total number of evicted keys.
    pub fn prune(&self) -> usize {
        self.minute.prune() + self.hour.prune() + self.day.prune()
    }

    pub fn snapshot(&self) -> RateLimiterSnapshot<K> {
        RateLimiterSnapshot {
            minute: self.minute.snapshot(),
            hour: self.hour.snapshot(),
            day: self.day.snapshot(),
        }
    }

    pub fn restore(&self, snapshot: RateLimiterSnapshot<K>) {
        let RateLimiterSnapshot { minute, hour, day } = snapshot;
        self.minute.restore(minute);
        self.hour.restore(hour);
        self.day.restore(day);
    }
}

struct Window<K> {
    buckets: Buckets,
    limit: usize,
    event_map: DashMap<K, BucketedEvents>,
}

impl<K> Window<K>
where
    K: PartialEq + Eq + Hash,
{
    fn new(duration: Duration, limit: usize) -> Self {
        Self {
            buckets: Buckets::new(duration),
            limit,
            event_map: DashMap::new(),
        }
    }

    fn snapshot(&self) -> WindowSnapshot<K>
    where
        K: Clone,
    {
        self.snapshot_at(SystemTime::now())
    }

    fn snapshot_at(&self, now: SystemTime) -> WindowSnapshot<K>
    where
        K: Clone,
    {
        let cutoff = self.buckets.cutoff_bucket(now);
        let mut events = HashMap::new();
        for entry in &self.event_map {
            let buckets: Vec<(EpochBucket, u32)> = entry
                .value()
                .buckets
                .iter()
                .filter(|(bucket, _)| *bucket >= cutoff)
                .copied()
                .collect();
            if !buckets.is_empty() {
                events.insert(entry.key().clone(), buckets);
            }
        }
        WindowSnapshot { events }
    }

    fn restore(&self, snapshot: WindowSnapshot<K>) {
        self.restore_at(snapshot, SystemTime::now());
    }

    fn restore_at(&self, snapshot: WindowSnapshot<K>, now: SystemTime) {
        let cutoff = self.buckets.cutoff_bucket(now);
        for (key, buckets) in snapshot.events {
            let filtered: VecDeque<(u64, u32)> = buckets
                .into_iter()
                .filter(|(bucket, _)| *bucket >= cutoff)
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

    /// Drop every key whose events have all aged out of the window.
    ///
    /// Returns the number of evicted keys. The count is advisory: concurrent traffic can add or
    /// remove keys while the pass runs, so it is only ever used for reporting.
    fn prune(&self) -> usize {
        let cutoff = self.buckets.cutoff_bucket(SystemTime::now());
        let before = self.event_map.len();
        self.event_map.retain(|_, events| {
            events.prune(cutoff);
            events.total > 0
        });
        before.saturating_sub(self.event_map.len())
    }

    fn check(&self, key: K) -> bool {
        self.check_at(key, SystemTime::now())
    }

    fn check_at(&self, key: K, now: SystemTime) -> bool {
        let now_bucket = self.buckets.now_bucket(now);
        let cutoff = self.buckets.cutoff_bucket(now);

        let mut entry = self
            .event_map
            .entry(key)
            .or_insert_with(|| BucketedEvents::with_capacity(DEFAULT_CAPACITY));
        entry.prune(cutoff);

        if entry.total < self.limit {
            entry.record(now_bucket);
            true
        } else {
            entry.evict_oldest();
            entry.record(now_bucket);
            false
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

    fn prune(&mut self, cutoff: u64) {
        while self
            .buckets
            .front()
            .is_some_and(|(bucket, _)| *bucket < cutoff)
        {
            if let Some((_, count)) = self.buckets.pop_front() {
                self.total = self.total.saturating_sub(count as usize);
            }
        }
    }

    fn record(&mut self, now_bucket: u64) {
        if let Some((bucket, count)) = self.buckets.back_mut()
            && *bucket == now_bucket
        {
            *count = count.saturating_add(1);
            self.total = self.total.saturating_add(1);
            return;
        }
        self.buckets.push_back((now_bucket, 1));
        self.total = self.total.saturating_add(1);
    }

    fn evict_oldest(&mut self) {
        if let Some((_, count)) = self.buckets.front_mut() {
            if *count > 1 {
                *count -= 1;
            } else {
                self.buckets.pop_front();
            }
            self.total = self.total.saturating_sub(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn test_now() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_hours(240)
    }

    #[test]
    fn check_under_limit() {
        let limiter = RateLimiter::new(RateLimits {
            minute: 10,
            hour: 100,
            day: 1000,
        });
        assert!(limiter.check(1u32).is_none());
        assert!(limiter.check(1u32).is_none());
    }

    #[test]
    fn check_at_limit() {
        let limiter = RateLimiter::new(RateLimits {
            minute: 2,
            hour: 100,
            day: 1000,
        });
        assert!(limiter.check(1u32).is_none());
        assert!(limiter.check(1u32).is_none());
        assert_eq!(limiter.check(1u32), Some(Interval::Minute));
    }

    #[test]
    fn check_eviction_slides_window() {
        let limiter = RateLimiter::new(RateLimits {
            minute: 2,
            hour: 100,
            day: 1000,
        });
        assert!(limiter.check(1u32).is_none());
        assert!(limiter.check(1u32).is_none());
        assert!(limiter.check(1u32).is_some());
        assert!(limiter.check(1u32).is_some());
    }

    #[test]
    fn multiple_keys_independent() {
        let limiter = RateLimiter::new(RateLimits {
            minute: 1,
            hour: 100,
            day: 1000,
        });
        assert!(limiter.check(1u32).is_none());
        assert!(limiter.check(2u32).is_none());
        assert!(limiter.check(1u32).is_some());
        assert!(limiter.check(2u32).is_some());
    }

    #[test]
    fn minute_limited_returns_minute_interval() {
        let limiter = RateLimiter::new(RateLimits {
            minute: 1,
            hour: 1000,
            day: 10000,
        });
        assert!(limiter.check(1u32).is_none());
        assert_eq!(limiter.check(1u32), Some(Interval::Minute));
    }

    #[test]
    fn all_windows_record_even_when_limited() {
        let limiter = RateLimiter::new(RateLimits {
            minute: 1,
            hour: 1000,
            day: 10000,
        });
        assert!(limiter.check(1u32).is_none());
        assert_eq!(limiter.check(1u32), Some(Interval::Minute));

        let snapshot = limiter.snapshot();
        assert!(snapshot.hour.events.contains_key(&1u32));
        assert!(snapshot.day.events.contains_key(&1u32));
    }

    #[test]
    fn snapshot_round_trip() {
        let limiter = RateLimiter::new(RateLimits {
            minute: 100,
            hour: 1000,
            day: 10000,
        });
        limiter.check(1u32);
        limiter.check(2u32);

        let snapshot = limiter.snapshot();

        let limiter2 = RateLimiter::new(RateLimits {
            minute: 100,
            hour: 1000,
            day: 10000,
        });
        limiter2.restore(snapshot);

        let snapshot2 = limiter2.snapshot();
        assert!(snapshot2.minute.events.contains_key(&1u32));
        assert!(snapshot2.minute.events.contains_key(&2u32));
    }

    /// Whether an event recorded at `event` still counts against a limit of one at `probe`.
    fn counted_at(window: Duration, event: SystemTime, probe: SystemTime) -> bool {
        let window = Window::new(window, 1);
        assert!(window.check_at(1u32, event));
        !window.check_at(1, probe)
    }

    // `test_now()` is a multiple of every bucket size used, so `event - offset` starts a bucket.
    #[test]
    fn event_counts_for_the_window_and_at_most_one_bucket_more() {
        for (window, bucket) in [
            (MINUTE, Duration::from_secs(5)),
            (HOUR, Duration::from_mins(5)),
            (DAY, Duration::from_hours(2)),
        ] {
            for offset in [
                Duration::ZERO,
                Duration::from_secs(1),
                bucket.saturating_sub(Duration::from_secs(1)),
            ] {
                let event = test_now() + offset;
                assert!(counted_at(
                    window,
                    event,
                    event + window - Duration::from_secs(1)
                ));
                assert!(counted_at(
                    window,
                    event,
                    event + window + bucket - Duration::from_secs(1) - offset
                ));
                assert!(!counted_at(window, event, event + window + bucket));
            }
        }
    }

    #[test]
    fn day_limit_reached_in_the_evening_releases_before_the_next_day_boundary() {
        let evening = test_now() + Duration::from_hours(20);
        let limited_at = |probe| {
            let window = Window::new(DAY, 100);
            for _ in 0..100 {
                assert!(window.check_at(1u32, evening));
            }
            !window.check_at(1, probe)
        };
        assert!(limited_at(evening + DAY - Duration::from_secs(1)));
        assert!(!limited_at(evening + DAY + Duration::from_hours(2)));
    }

    #[test]
    fn rejected_check_evicts_the_oldest_and_records() {
        let window = Window::new(MINUTE, 2);
        let start = test_now();
        assert!(window.check_at(1u32, start));
        assert!(window.check_at(1, start));
        let rejected = start + Duration::from_secs(30);
        for _ in 0..3 {
            assert!(!window.check_at(1, rejected));
        }
        // No check result can observe the eviction: it only keeps the total at the limit.
        assert_eq!(window.event_map.get(&1).unwrap().total, 2);

        assert!(!window.check_at(1, start + MINUTE + Duration::from_secs(5)));
    }

    #[test]
    fn snapshot_round_trip_keeps_the_limit() {
        let now = test_now();
        let window = Window::new(MINUTE, 2);
        assert!(window.check_at(1u32, now - Duration::from_secs(50)));
        assert!(window.check_at(1, now));

        let restored = Window::new(MINUTE, 2);
        restored.restore_at(window.snapshot_at(now), now);
        assert!(!restored.check_at(1, now));
    }

    #[test]
    fn restore_after_time_advances_drops_expired_buckets() {
        let now = test_now();
        let window = Window::new(MINUTE, 2);
        assert!(window.check_at(1u32, now - Duration::from_secs(50)));
        assert!(window.check_at(1, now));
        let snapshot = window.snapshot_at(now);

        let later = now + Duration::from_secs(15);
        let restored = Window::new(MINUTE, 2);
        restored.restore_at(snapshot, later);
        assert!(restored.check_at(1, later));
        assert!(!restored.check_at(1, later));
    }

    #[test]
    fn prune_removes_stale_keys() {
        let window: Window<u32> = Window::new(Duration::from_mins(1), 100);
        window.event_map.insert(
            1,
            BucketedEvents {
                buckets: VecDeque::from([(stale_bucket(MINUTE), 3)]),
                total: 3,
            },
        );
        assert!(window.event_map.contains_key(&1));

        window.prune();
        assert!(!window.event_map.contains_key(&1));
    }

    fn stale_bucket(window: Duration) -> EpochBucket {
        Buckets::new(window).now_bucket(test_now() - Duration::from_mins(2))
    }

    fn stale_events(window: Duration) -> BucketedEvents {
        BucketedEvents {
            buckets: VecDeque::from([(stale_bucket(window), 3)]),
            total: 3,
        }
    }

    #[test]
    fn prune_empty_returns_zero() {
        let window: Window<u32> = Window::new(Duration::from_mins(1), 100);
        assert_eq!(window.prune(), 0);
    }

    #[test]
    fn prune_returns_evicted_count() {
        let window: Window<u32> = Window::new(Duration::from_mins(1), 100);
        window.event_map.insert(1, stale_events(MINUTE));
        window.event_map.insert(2, stale_events(MINUTE));

        assert_eq!(window.prune(), 2);
        assert!(window.event_map.is_empty());
    }

    #[test]
    fn prune_does_not_count_live_keys() {
        let window: Window<u32> = Window::new(Duration::from_mins(1), 100);
        window.event_map.insert(1, stale_events(MINUTE));
        assert!(window.check(2));

        assert_eq!(window.prune(), 1);
        assert!(!window.event_map.contains_key(&1));
        assert!(window.event_map.contains_key(&2));
    }

    #[test]
    fn prune_returns_zero_when_all_keys_live() {
        let window: Window<u32> = Window::new(Duration::from_mins(1), 100);
        assert!(window.check(1));
        assert!(window.check(2));

        assert_eq!(window.prune(), 0);
        assert_eq!(window.event_map.len(), 2);
    }

    #[test]
    fn rate_limiter_prune_sums_windows() {
        let limiter: RateLimiter<u32> = RateLimiter::new(RateLimits {
            minute: 100,
            hour: 1000,
            day: 10000,
        });
        limiter.minute.event_map.insert(1, stale_events(MINUTE));
        limiter.hour.event_map.insert(1, stale_events(HOUR));
        limiter.hour.event_map.insert(2, stale_events(HOUR));
        limiter.day.event_map.insert(1, stale_events(DAY));

        assert_eq!(limiter.prune(), 4);
    }

    #[test]
    fn bucket_merging() {
        let mut events = BucketedEvents::with_capacity(1);
        events.record(100);
        events.record(100);
        events.record(101);
        assert_eq!(events.total, 3);
        assert_eq!(events.buckets.len(), 2);
        assert_eq!(events.buckets[0], (100, 2));
        assert_eq!(events.buckets[1], (101, 1));
    }

    #[test]
    fn evict_decrements_count() {
        let mut events = BucketedEvents::with_capacity(1);
        events.record(100);
        events.record(100);
        events.record(100);
        assert_eq!(events.total, 3);

        events.evict_oldest();
        assert_eq!(events.total, 2);
        assert_eq!(events.buckets[0], (100, 2));

        events.evict_oldest();
        assert_eq!(events.total, 1);
        assert_eq!(events.buckets[0], (100, 1));

        events.evict_oldest();
        assert_eq!(events.total, 0);
        assert!(events.buckets.is_empty());
    }

    #[test]
    fn prune_updates_total() {
        let mut events = BucketedEvents::with_capacity(1);
        events.record(100);
        events.record(100);
        events.record(101);
        assert_eq!(events.total, 3);

        events.prune(101);
        assert_eq!(events.total, 1);
        assert_eq!(events.buckets.len(), 1);
        assert_eq!(events.buckets[0], (101, 1));
    }
}
