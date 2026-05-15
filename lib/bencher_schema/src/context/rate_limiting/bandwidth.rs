use std::{
    collections::{HashMap, VecDeque},
    time::SystemTime,
};

use bencher_json::{OrganizationUuid, Priority, system::config::JsonOciBandwidth};
use dashmap::DashMap;
use dropshot::HttpError;

use crate::{
    context::RateLimitingError, error::too_many_requests, model::organization::QueryOrganization,
};

use super::DAY;
use super::epoch_bucket;
use super::snapshot::{BandwidthRateLimiterSnapshot, EpochBucket};

const DEFAULT_UNCLAIMED_BANDWIDTH: u64 = 1 << 30;
const DEFAULT_FREE_BANDWIDTH: u64 = 10 << 30;
const DEFAULT_PLUS_BANDWIDTH: u64 = 100 << 30;

const BYTES_PER_GIB: u64 = 1 << 30;

pub(super) struct BandwidthRateLimiter {
    event_map: DashMap<OrganizationUuid, BucketedBandwidth>,
    unclaimed_limit: u64,
    free_limit: u64,
    plus_limit: u64,
}

impl Default for BandwidthRateLimiter {
    fn default() -> Self {
        Self {
            event_map: DashMap::new(),
            unclaimed_limit: DEFAULT_UNCLAIMED_BANDWIDTH,
            free_limit: DEFAULT_FREE_BANDWIDTH,
            plus_limit: DEFAULT_PLUS_BANDWIDTH,
        }
    }
}

impl From<JsonOciBandwidth> for BandwidthRateLimiter {
    fn from(json: JsonOciBandwidth) -> Self {
        let JsonOciBandwidth {
            unclaimed,
            free,
            plus,
        } = json;
        Self {
            event_map: DashMap::new(),
            unclaimed_limit: unclaimed.unwrap_or(DEFAULT_UNCLAIMED_BANDWIDTH),
            free_limit: free.unwrap_or(DEFAULT_FREE_BANDWIDTH),
            plus_limit: plus.unwrap_or(DEFAULT_PLUS_BANDWIDTH),
        }
    }
}

impl BandwidthRateLimiter {
    pub fn max() -> Self {
        Self {
            event_map: DashMap::new(),
            unclaimed_limit: u64::MAX,
            free_limit: u64::MAX,
            plus_limit: u64::MAX,
        }
    }

    fn cutoff_bucket(now: SystemTime) -> u64 {
        let now_secs = now
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        epoch_bucket(now_secs.saturating_sub(DAY.as_secs()), DAY.as_secs())
    }

    fn now_bucket(now: SystemTime) -> u64 {
        let now_secs = now
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        epoch_bucket(now_secs, DAY.as_secs())
    }

    pub fn snapshot(&self) -> BandwidthRateLimiterSnapshot {
        let cutoff = Self::cutoff_bucket(SystemTime::now());
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
        BandwidthRateLimiterSnapshot { events }
    }

    pub fn restore(&self, snapshot: BandwidthRateLimiterSnapshot) {
        let cutoff = Self::cutoff_bucket(SystemTime::now());
        for (org_uuid, buckets) in snapshot.events {
            let filtered: VecDeque<(u64, u64)> = buckets
                .into_iter()
                .filter(|(bucket, _)| *bucket >= cutoff)
                .collect();
            let total_bytes: u64 = filtered.iter().map(|(_, b)| b).sum();
            if total_bytes > 0 {
                self.event_map.insert(
                    org_uuid,
                    BucketedBandwidth {
                        buckets: filtered,
                        total_bytes,
                    },
                );
            }
        }
    }

    pub fn prune(&self) {
        let cutoff = Self::cutoff_bucket(SystemTime::now());
        self.event_map.retain(|_, bw| {
            bw.prune(cutoff);
            bw.total_bytes > 0
        });
    }

    fn limit_for_priority(&self, priority: Priority) -> u64 {
        match priority {
            Priority::Unclaimed => self.unclaimed_limit,
            Priority::Free => self.free_limit,
            Priority::Plus => self.plus_limit,
        }
    }

    pub fn check(
        &self,
        org_uuid: OrganizationUuid,
        priority: Priority,
        organization: &QueryOrganization,
    ) -> Result<(), HttpError> {
        self.check_at(org_uuid, priority, organization, SystemTime::now())
    }

    fn check_at(
        &self,
        org_uuid: OrganizationUuid,
        priority: Priority,
        organization: &QueryOrganization,
        now: SystemTime,
    ) -> Result<(), HttpError> {
        let limit = self.limit_for_priority(priority);
        let cutoff = Self::cutoff_bucket(now);

        let total_bytes = if let Some(mut bw) = self.event_map.get_mut(&org_uuid) {
            bw.prune(cutoff);
            bw.total_bytes
        } else {
            0
        };

        if total_bytes >= limit {
            Err(too_many_requests(RateLimitingError::OciBandwidth {
                organization: organization.clone(),
                limit_gib: limit.saturating_div(BYTES_PER_GIB),
            }))
        } else {
            Ok(())
        }
    }

    pub fn record(&self, org_uuid: OrganizationUuid, bytes: u64) {
        self.record_at(org_uuid, bytes, SystemTime::now());
    }

    fn record_at(&self, org_uuid: OrganizationUuid, bytes: u64, now: SystemTime) {
        if bytes == 0 {
            return;
        }
        let now_bucket = Self::now_bucket(now);
        self.event_map
            .entry(org_uuid)
            .or_default()
            .record(now_bucket, bytes);
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
    use std::time::{Duration, SystemTime};

    use super::*;

    fn test_now() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(86_400 * 3)
    }

    fn org_uuid() -> OrganizationUuid {
        OrganizationUuid::from(uuid::Uuid::nil())
    }

    fn org() -> QueryOrganization {
        use crate::model::organization::OrganizationId;
        use bencher_json::{DateTime, OrganizationSlug, ResourceName};
        QueryOrganization {
            id: OrganizationId::from_raw(1),
            uuid: org_uuid(),
            name: "test-org".parse::<ResourceName>().unwrap(),
            slug: "test-org".parse::<OrganizationSlug>().unwrap(),
            license: None,
            created: DateTime::TEST,
            modified: DateTime::TEST,
            deleted: None,
        }
    }

    #[test]
    fn basic_tracking() {
        let limiter = BandwidthRateLimiter {
            event_map: DashMap::new(),
            unclaimed_limit: 1000,
            free_limit: 10_000,
            plus_limit: 100_000,
        };

        let now = test_now();
        let org_uuid = org_uuid();
        let org = org();

        limiter
            .check_at(org_uuid, Priority::Unclaimed, &org, now)
            .unwrap();

        limiter.record_at(org_uuid, 500, now);
        limiter
            .check_at(org_uuid, Priority::Unclaimed, &org, now)
            .unwrap();

        limiter.record_at(org_uuid, 600, now);
        let result = limiter.check_at(org_uuid, Priority::Unclaimed, &org, now);
        assert!(result.is_err());
    }

    #[test]
    fn tier_limits() {
        let limiter = BandwidthRateLimiter {
            event_map: DashMap::new(),
            unclaimed_limit: 100,
            free_limit: 1000,
            plus_limit: 10_000,
        };

        let now = test_now();
        let org_uuid = org_uuid();
        let org = org();

        limiter.record_at(org_uuid, 500, now);

        assert!(
            limiter
                .check_at(org_uuid, Priority::Unclaimed, &org, now)
                .is_err()
        );
        assert!(
            limiter
                .check_at(org_uuid, Priority::Free, &org, now)
                .is_ok()
        );
        assert!(
            limiter
                .check_at(org_uuid, Priority::Plus, &org, now)
                .is_ok()
        );
    }

    #[test]
    fn window_cleanup() {
        let limiter = BandwidthRateLimiter {
            event_map: DashMap::new(),
            unclaimed_limit: 100,
            free_limit: 1000,
            plus_limit: 10_000,
        };

        let now = test_now();
        let org_uuid = org_uuid();
        let org = org();

        let old_bucket = epoch_bucket(
            (now - Duration::from_secs(25 * 60 * 60))
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            DAY.as_secs(),
        );
        limiter.event_map.insert(
            org_uuid,
            BucketedBandwidth {
                buckets: VecDeque::from([(old_bucket, 500)]),
                total_bytes: 500,
            },
        );

        limiter
            .check_at(org_uuid, Priority::Unclaimed, &org, now)
            .unwrap();
    }

    #[test]
    fn max_mode() {
        let limiter = BandwidthRateLimiter::max();
        let now = test_now();
        let org_uuid = org_uuid();
        let org = org();

        limiter.record_at(org_uuid, u64::MAX.saturating_div(2), now);
        limiter
            .check_at(org_uuid, Priority::Unclaimed, &org, now)
            .unwrap();
    }

    #[test]
    fn zero_bytes_not_recorded() {
        let limiter = BandwidthRateLimiter::default();
        let now = test_now();
        let org_uuid = org_uuid();

        limiter.record_at(org_uuid, 0, now);
        assert!(!limiter.event_map.contains_key(&org_uuid));
    }
}
