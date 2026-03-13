use std::{collections::VecDeque, time::SystemTime};

use bencher_json::system::config::JsonOciBandwidth;
use dashmap::DashMap;
use dropshot::HttpError;

use crate::{
    context::RateLimitingError,
    error::too_many_requests,
    model::organization::{OrganizationId, QueryOrganization},
};

use super::{DAY, OciBandwidthTier};

/// 1 GiB in bytes
const DEFAULT_UNCLAIMED_BANDWIDTH: u64 = 1 << 30;
/// 10 GiB in bytes
const DEFAULT_FREE_BANDWIDTH: u64 = 10 << 30;
/// 100 GiB in bytes
const DEFAULT_PLUS_BANDWIDTH: u64 = 100 << 30;

/// Bytes per GiB
const BYTES_PER_GIB: u64 = 1 << 30;

pub(super) struct BandwidthRateLimiter {
    event_map: DashMap<OrganizationId, VecDeque<(SystemTime, u64)>>,
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

    fn limit_for_tier(&self, tier: OciBandwidthTier) -> u64 {
        match tier {
            OciBandwidthTier::Unclaimed => self.unclaimed_limit,
            OciBandwidthTier::Free => self.free_limit,
            OciBandwidthTier::Plus => self.plus_limit,
        }
    }

    pub fn check(
        &self,
        org_id: OrganizationId,
        tier: OciBandwidthTier,
        organization: &QueryOrganization,
    ) -> Result<(), HttpError> {
        self.check_at(org_id, tier, organization, SystemTime::now())
    }

    fn check_at(
        &self,
        org_id: OrganizationId,
        tier: OciBandwidthTier,
        organization: &QueryOrganization,
        now: SystemTime,
    ) -> Result<(), HttpError> {
        let limit = self.limit_for_tier(tier);
        let cutoff = now - DAY;

        // Clean up old entries across all orgs
        self.event_map.retain(|_, events| {
            while events.front().is_some_and(|(time, _)| *time < cutoff) {
                events.pop_front();
            }
            !events.is_empty()
        });

        // Sum bytes in the current 24h window (saturating to avoid overflow)
        let total_bytes: u64 = self.event_map.get(&org_id).map_or(0, |events| {
            events
                .iter()
                .fold(0u64, |acc, (_, bytes)| acc.saturating_add(*bytes))
        });

        if total_bytes >= limit {
            Err(too_many_requests(RateLimitingError::OciBandwidth {
                organization: organization.clone(),
                limit_gib: limit.saturating_div(BYTES_PER_GIB),
            }))
        } else {
            Ok(())
        }
    }

    pub fn record(&self, org_id: OrganizationId, bytes: u64) {
        self.record_at(org_id, bytes, SystemTime::now());
    }

    fn record_at(&self, org_id: OrganizationId, bytes: u64, now: SystemTime) {
        if bytes == 0 {
            return;
        }
        self.event_map
            .entry(org_id)
            .or_default()
            .push_back((now, bytes));
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use super::*;

    fn test_now() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(100_000)
    }

    fn org_id() -> OrganizationId {
        OrganizationId::from_raw(1)
    }

    fn org() -> QueryOrganization {
        use bencher_json::{DateTime, OrganizationSlug, OrganizationUuid, ResourceName};
        QueryOrganization {
            id: org_id(),
            uuid: OrganizationUuid::from(uuid::Uuid::nil()),
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
        let org_id = org_id();
        let org = org();

        // Should be under limit
        limiter
            .check_at(org_id, OciBandwidthTier::Unclaimed, &org, now)
            .unwrap();

        // Record some bytes
        limiter.record_at(org_id, 500, now);
        limiter
            .check_at(org_id, OciBandwidthTier::Unclaimed, &org, now)
            .unwrap();

        // Record more to exceed limit
        limiter.record_at(org_id, 600, now);
        let result = limiter.check_at(org_id, OciBandwidthTier::Unclaimed, &org, now);
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
        let org_id = org_id();
        let org = org();

        // Record 500 bytes - over unclaimed, under free and plus
        limiter.record_at(org_id, 500, now);

        assert!(
            limiter
                .check_at(org_id, OciBandwidthTier::Unclaimed, &org, now)
                .is_err()
        );
        assert!(
            limiter
                .check_at(org_id, OciBandwidthTier::Free, &org, now)
                .is_ok()
        );
        assert!(
            limiter
                .check_at(org_id, OciBandwidthTier::Plus, &org, now)
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
        let org_id = org_id();
        let org = org();

        // Insert an old entry (more than 24h ago)
        let old_time = now - Duration::from_secs(25 * 60 * 60);
        limiter
            .event_map
            .entry(org_id)
            .or_default()
            .push_back((old_time, 500));

        // Should be under limit because old entries get cleaned up
        limiter
            .check_at(org_id, OciBandwidthTier::Unclaimed, &org, now)
            .unwrap();
    }

    #[test]
    fn max_mode() {
        let limiter = BandwidthRateLimiter::max();
        let now = test_now();
        let org_id = org_id();
        let org = org();

        // Even huge amounts should be under limit
        limiter.record_at(org_id, u64::MAX.saturating_div(2), now);
        limiter
            .check_at(org_id, OciBandwidthTier::Unclaimed, &org, now)
            .unwrap();
    }

    #[test]
    fn zero_bytes_not_recorded() {
        let limiter = BandwidthRateLimiter::default();
        let now = test_now();
        let org_id = org_id();

        limiter.record_at(org_id, 0, now);
        assert!(!limiter.event_map.contains_key(&org_id));
    }
}
