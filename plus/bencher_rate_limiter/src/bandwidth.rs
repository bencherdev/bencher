use std::{
    collections::{HashMap, VecDeque},
    hash::Hash,
    time::{Duration, SystemTime},
};

use dashmap::DashMap;

use crate::epoch_bucket;
use crate::snapshot::{BandwidthSnapshot, EpochBucket};

pub struct BandwidthLimiter<K> {
    window: Duration,
    event_map: DashMap<K, BucketedBandwidth>,
}

impl<K> BandwidthLimiter<K>
where
    K: PartialEq + Eq + Hash + Clone + Copy,
{
    pub fn new(window: Duration) -> Self {
        Self {
            window,
            event_map: DashMap::new(),
        }
    }

    fn cutoff_bucket(&self, now: SystemTime) -> u64 {
        let now_secs = now
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        epoch_bucket(
            now_secs.saturating_sub(self.window.as_secs()),
            self.window.as_secs(),
        )
    }

    fn now_bucket(&self, now: SystemTime) -> u64 {
        let now_secs = now
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        epoch_bucket(now_secs, self.window.as_secs())
    }

    pub fn check(&self, key: &K, limit: u64) -> bool {
        self.check_at(key, limit, SystemTime::now())
    }

    pub fn check_at(&self, key: &K, limit: u64, now: SystemTime) -> bool {
        let cutoff = self.cutoff_bucket(now);

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
        let now_bucket = self.now_bucket(now);
        self.event_map
            .entry(key)
            .or_default()
            .record(now_bucket, bytes);
    }

    pub fn prune(&self) {
        let cutoff = self.cutoff_bucket(SystemTime::now());
        self.event_map.retain(|_, bw| {
            bw.prune(cutoff);
            bw.total_bytes > 0
        });
    }

    pub fn snapshot(&self) -> BandwidthSnapshot<K> {
        let cutoff = self.cutoff_bucket(SystemTime::now());
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
        let cutoff = self.cutoff_bucket(SystemTime::now());
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
#[expect(clippy::indexing_slicing, clippy::get_unwrap)]
mod tests {
    use std::time::Duration;

    use super::*;

    const DAY: Duration = Duration::from_secs(86_400);

    fn test_now() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(86_400 * 3)
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
        let old = now - Duration::from_secs(25 * 60 * 60);
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

    #[test]
    fn snapshot_round_trip() {
        let limiter = BandwidthLimiter::new(DAY);
        limiter.record(1u32, 1024);
        limiter.record(1, 2048);

        let snapshot = limiter.snapshot();
        let entries = snapshot.events.get(&1).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].1, 3072);

        let limiter2 = BandwidthLimiter::new(DAY);
        limiter2.restore(snapshot);

        let snapshot2 = limiter2.snapshot();
        let entries2 = snapshot2.events.get(&1).unwrap();
        assert_eq!(entries2.len(), 1);
        assert_eq!(entries2[0].1, 3072);
    }

    #[test]
    fn restore_filters_expired() {
        let limiter = BandwidthLimiter::<u32>::new(DAY);
        let old_bucket = epoch_bucket(
            (test_now() - Duration::from_secs(25 * 60 * 60))
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            DAY.as_secs(),
        );
        let snapshot = BandwidthSnapshot {
            events: HashMap::from([(1u32, vec![(old_bucket, 500)])]),
        };
        limiter.restore(snapshot);
        let snapshot2 = limiter.snapshot();
        assert!(snapshot2.events.is_empty());
    }

    #[test]
    fn prune_removes_stale_keys() {
        let limiter = BandwidthLimiter::new(DAY);
        let old = test_now() - Duration::from_secs(25 * 60 * 60);
        limiter.record_at(1u32, 500, old);
        assert!(limiter.event_map.contains_key(&1));

        limiter.prune();
        assert!(!limiter.event_map.contains_key(&1));
    }

    #[test]
    fn bucket_merging() {
        let limiter = BandwidthLimiter::new(DAY);
        limiter.record(1u32, 100);
        limiter.record(1, 200);

        let snapshot = limiter.snapshot();
        let entries = snapshot.events.get(&1).unwrap();
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
