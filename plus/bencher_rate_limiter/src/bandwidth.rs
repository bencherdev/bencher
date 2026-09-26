use std::{
    collections::{HashMap, VecDeque},
    hash::Hash,
    time::{Duration, SystemTime},
};

use dashmap::DashMap;

use crate::Buckets;
use crate::snapshot::{BandwidthSnapshot, EpochBucket};

pub struct BandwidthLimiter<K> {
    buckets: Buckets,
    event_map: DashMap<K, BucketedBandwidth>,
}

impl<K> BandwidthLimiter<K>
where
    K: PartialEq + Eq + Hash + Clone + Copy,
{
    pub fn new(window: Duration) -> Self {
        Self {
            buckets: Buckets::new(window),
            event_map: DashMap::new(),
        }
    }

    pub fn check(&self, key: &K, limit: u64) -> bool {
        self.check_at(key, limit, SystemTime::now())
    }

    pub fn check_at(&self, key: &K, limit: u64, now: SystemTime) -> bool {
        let cutoff = self.buckets.cutoff_bucket(now);

        let total_bytes = if let Some(mut bw) = self.event_map.get_mut(key) {
            bw.prune(cutoff);
            bw.total_bytes
        } else {
            0
        };

        total_bytes < limit
    }

    pub fn record(&self, key: K, bytes: u64) {
        self.record_at(key, bytes, SystemTime::now());
    }

    pub fn record_at(&self, key: K, bytes: u64, now: SystemTime) {
        if bytes == 0 {
            return;
        }
        let now_bucket = self.buckets.now_bucket(now);
        self.event_map
            .entry(key)
            .or_default()
            .record(now_bucket, bytes);
    }

    /// Drop every key whose bandwidth has all aged out of the window.
    ///
    /// Returns the number of evicted keys. The count is advisory: concurrent traffic can add or
    /// remove keys while the pass runs, so it is only ever used for reporting.
    pub fn prune(&self) -> usize {
        let cutoff = self.buckets.cutoff_bucket(SystemTime::now());
        let before = self.event_map.len();
        self.event_map.retain(|_, bw| {
            bw.prune(cutoff);
            bw.total_bytes > 0
        });
        before.saturating_sub(self.event_map.len())
    }

    pub fn snapshot(&self) -> BandwidthSnapshot<K> {
        self.snapshot_at(SystemTime::now())
    }

    fn snapshot_at(&self, now: SystemTime) -> BandwidthSnapshot<K> {
        let cutoff = self.buckets.cutoff_bucket(now);
        let mut events = HashMap::new();
        for entry in &self.event_map {
            let buckets: Vec<(EpochBucket, u64)> = entry
                .value()
                .buckets
                .iter()
                .filter(|(bucket, _)| *bucket >= cutoff)
                .copied()
                .collect();
            if !buckets.is_empty() {
                events.insert(*entry.key(), buckets);
            }
        }
        BandwidthSnapshot { events }
    }

    pub fn restore(&self, snapshot: BandwidthSnapshot<K>) {
        self.restore_at(snapshot, SystemTime::now());
    }

    fn restore_at(&self, snapshot: BandwidthSnapshot<K>, now: SystemTime) {
        let cutoff = self.buckets.cutoff_bucket(now);
        for (key, buckets) in snapshot.events {
            let filtered: VecDeque<(u64, u64)> = buckets
                .into_iter()
                .filter(|(bucket, _)| *bucket >= cutoff)
                .collect();
            let total_bytes: u64 = filtered.iter().map(|(_, b)| b).sum();
            if total_bytes > 0 {
                self.event_map.insert(
                    key,
                    BucketedBandwidth {
                        buckets: filtered,
                        total_bytes,
                    },
                );
            }
        }
    }
}

#[derive(Default)]
struct BucketedBandwidth {
    buckets: VecDeque<(u64, u64)>,
    total_bytes: u64,
}

impl BucketedBandwidth {
    fn prune(&mut self, cutoff: u64) {
        while self
            .buckets
            .front()
            .is_some_and(|(bucket, _)| *bucket < cutoff)
        {
            if let Some((_, bytes)) = self.buckets.pop_front() {
                self.total_bytes = self.total_bytes.saturating_sub(bytes);
            }
        }
    }

    fn record(&mut self, now_bucket: u64, bytes: u64) {
        if let Some((bucket, bucket_bytes)) = self.buckets.back_mut()
            && *bucket == now_bucket
        {
            *bucket_bytes = bucket_bytes.saturating_add(bytes);
            self.total_bytes = self.total_bytes.saturating_add(bytes);
            return;
        }
        self.buckets.push_back((now_bucket, bytes));
        self.total_bytes = self.total_bytes.saturating_add(bytes);
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::DAY;

    fn test_now() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_hours(72)
    }

    #[test]
    fn check_under_limit() {
        let limiter = BandwidthLimiter::new(DAY);
        let now = test_now();
        limiter.record_at(1u32, 500, now);
        assert!(limiter.check_at(&1, 1000, now));
    }

    #[test]
    fn check_over_limit() {
        let limiter = BandwidthLimiter::new(DAY);
        let now = test_now();
        limiter.record_at(1u32, 1100, now);
        assert!(!limiter.check_at(&1, 1000, now));
    }

    #[test]
    fn basic_tracking() {
        let limiter = BandwidthLimiter::new(DAY);
        let now = test_now();
        assert!(limiter.check_at(&1u32, 1000, now));

        limiter.record_at(1, 500, now);
        assert!(limiter.check_at(&1, 1000, now));

        limiter.record_at(1, 600, now);
        assert!(!limiter.check_at(&1, 1000, now));
    }

    #[test]
    fn window_cleanup() {
        let limiter = BandwidthLimiter::new(DAY);
        let now = test_now();
        let old = now - (DAY + Duration::from_hours(2));
        limiter.record_at(1u32, 500, old);
        assert!(limiter.check_at(&1, 100, now));
    }

    #[test]
    fn zero_bytes_not_recorded() {
        let limiter = BandwidthLimiter::new(DAY);
        let now = test_now();
        limiter.record_at(1u32, 0, now);
        assert!(!limiter.event_map.contains_key(&1));
    }

    /// Whether one byte recorded at `event` still counts against a one byte limit at `probe`.
    fn counted_at(window: Duration, event: SystemTime, probe: SystemTime) -> bool {
        let limiter = BandwidthLimiter::new(window);
        limiter.record_at(1u32, 1, event);
        !limiter.check_at(&1, 1, probe)
    }

    // `test_now()` is a multiple of every bucket size used, so `event - offset` starts a bucket.
    fn assert_window_bound(window: Duration, bucket: Duration) {
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

    #[test]
    fn event_counts_for_the_window_and_at_most_one_bucket_more() {
        assert_window_bound(DAY, Duration::from_hours(2));
    }

    #[test]
    fn window_not_divisible_by_twelve() {
        assert_window_bound(Duration::from_secs(100), Duration::from_secs(8));
    }

    #[test]
    fn window_shorter_than_twelve_seconds() {
        assert_window_bound(Duration::from_secs(5), Duration::from_secs(1));
    }

    #[test]
    fn limit_reached_in_the_evening_releases_before_the_next_day_boundary() {
        let limiter = BandwidthLimiter::new(DAY);
        let evening = test_now() + Duration::from_hours(20);
        limiter.record_at(1u32, 1000, evening);
        assert!(!limiter.check_at(&1, 1000, evening + DAY - Duration::from_secs(1)));
        assert!(limiter.check_at(&1, 1000, evening + DAY + Duration::from_hours(2)));
    }

    #[test]
    fn snapshot_round_trip_keeps_the_limit() {
        let now = test_now();
        let limiter = BandwidthLimiter::new(DAY);
        limiter.record_at(1u32, 1024, now - Duration::from_hours(20));
        limiter.record_at(1, 2048, now);

        let restored = BandwidthLimiter::new(DAY);
        restored.restore_at(limiter.snapshot_at(now), now);
        assert!(restored.check_at(&1, 3073, now));
        assert!(!restored.check_at(&1, 3072, now));
    }

    #[test]
    fn restore_after_time_advances_drops_expired_buckets() {
        let now = test_now();
        let limiter = BandwidthLimiter::new(DAY);
        limiter.record_at(1u32, 500, now - Duration::from_hours(20));
        limiter.record_at(1, 200, now);
        let snapshot = limiter.snapshot_at(now);

        let later = now + Duration::from_hours(6);
        let restored = BandwidthLimiter::new(DAY);
        restored.restore_at(snapshot, later);
        assert!(restored.check_at(&1, 201, later));
        assert!(!restored.check_at(&1, 200, later));
    }

    #[test]
    fn prune_removes_stale_keys() {
        let limiter = BandwidthLimiter::new(DAY);
        let old = test_now() - Duration::from_hours(25);
        limiter.record_at(1u32, 500, old);
        assert!(limiter.event_map.contains_key(&1));

        limiter.prune();
        assert!(!limiter.event_map.contains_key(&1));
    }

    #[test]
    fn prune_empty_returns_zero() {
        let limiter: BandwidthLimiter<u32> = BandwidthLimiter::new(DAY);
        assert_eq!(limiter.prune(), 0);
    }

    #[test]
    fn prune_returns_evicted_count() {
        let limiter = BandwidthLimiter::new(DAY);
        let old = test_now() - Duration::from_hours(25);
        limiter.record_at(1u32, 500, old);
        limiter.record_at(2u32, 500, old);

        assert_eq!(limiter.prune(), 2);
        assert!(limiter.event_map.is_empty());
    }

    #[test]
    fn prune_does_not_count_live_keys() {
        let limiter = BandwidthLimiter::new(DAY);
        let old = test_now() - Duration::from_hours(25);
        limiter.record_at(1u32, 500, old);
        limiter.record(2u32, 500);

        assert_eq!(limiter.prune(), 1);
        assert!(!limiter.event_map.contains_key(&1));
        assert!(limiter.event_map.contains_key(&2));
    }

    #[test]
    fn prune_returns_zero_when_all_keys_live() {
        let limiter = BandwidthLimiter::new(DAY);
        limiter.record(1u32, 500);
        limiter.record(2u32, 500);

        assert_eq!(limiter.prune(), 0);
        assert_eq!(limiter.event_map.len(), 2);
    }

    #[test]
    fn bucket_merging() {
        let limiter = BandwidthLimiter::new(DAY);
        let now = test_now();
        limiter.record_at(1u32, 100, now);
        limiter.record_at(1, 200, now);

        let snapshot = limiter.snapshot_at(now);
        let entries = &snapshot.events[&1];
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].1, 300);
    }

    #[test]
    fn multiple_keys_independent() {
        let limiter = BandwidthLimiter::new(DAY);
        let now = test_now();
        limiter.record_at(1u32, 500, now);
        limiter.record_at(2u32, 200, now);

        assert!(!limiter.check_at(&1, 400, now));
        assert!(limiter.check_at(&2, 400, now));
    }
}
