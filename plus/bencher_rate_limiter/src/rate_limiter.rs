use std::{
    collections::{HashMap, VecDeque},
    hash::Hash,
    time::{Duration, SystemTime},
};

use dashmap::DashMap;

use crate::epoch_bucket;
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

    pub fn prune(&self) {
        self.minute.prune();
        self.hour.prune();
        self.day.prune();
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
    duration: Duration,
    limit: usize,
    event_map: DashMap<K, BucketedEvents>,
}

impl<K> Window<K>
where
    K: PartialEq + Eq + Hash,
{
    fn new(duration: Duration, limit: usize) -> Self {
        Self {
            duration,
            limit,
            event_map: DashMap::new(),
        }
    }

    fn now_and_cutoff(&self) -> (u64, u64) {
        Self::now_and_cutoff_at(self.duration, SystemTime::now())
    }

    fn now_and_cutoff_at(duration: Duration, now: SystemTime) -> (u64, u64) {
        let now_secs = now
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let bucket_secs = duration.as_secs();
        let now_bucket = epoch_bucket(now_secs, bucket_secs);
        let cutoff_bucket = epoch_bucket(now_secs.saturating_sub(bucket_secs), bucket_secs);
        (now_bucket, cutoff_bucket)
    }

    fn snapshot(&self) -> WindowSnapshot<K>
    where
        K: Clone,
    {
        let (_, cutoff) = self.now_and_cutoff();
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
        let (_, cutoff) = self.now_and_cutoff();
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

    fn prune(&self) {
        let (_, cutoff) = self.now_and_cutoff();
        self.event_map.retain(|_, events| {
            events.prune(cutoff);
            events.total > 0
        });
    }

    fn check(&self, key: K) -> bool {
        let (now_bucket, cutoff) = self.now_and_cutoff();

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

    #[test]
    fn restore_filters_expired() {
        let limiter = RateLimiter::new(RateLimits {
            minute: 100,
            hour: 1000,
            day: 10000,
        });

        let old_bucket = epoch_bucket(
            test_now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs()
                - 120,
            60,
        );
        let snapshot = RateLimiterSnapshot {
            minute: WindowSnapshot {
                events: HashMap::from([(1u32, vec![(old_bucket, 5)])]),
            },
            hour: WindowSnapshot {
                events: HashMap::new(),
            },
            day: WindowSnapshot {
                events: HashMap::new(),
            },
        };
        limiter.restore(snapshot);

        let snapshot2 = limiter.snapshot();
        assert!(snapshot2.minute.events.is_empty());
    }

    #[test]
    fn prune_removes_stale_keys() {
        let window: Window<u32> = Window::new(Duration::from_mins(1), 100);
        let old_bucket = epoch_bucket(
            test_now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs()
                - 120,
            60,
        );
        window.event_map.insert(
            1,
            BucketedEvents {
                buckets: VecDeque::from([(old_bucket, 3)]),
                total: 3,
            },
        );
        assert!(window.event_map.contains_key(&1));

        window.prune();
        assert!(!window.event_map.contains_key(&1));
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
