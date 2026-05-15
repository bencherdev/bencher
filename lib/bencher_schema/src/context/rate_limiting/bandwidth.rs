use std::time::{Duration, SystemTime};

use bencher_json::{OrganizationUuid, Priority, system::config::JsonOciBandwidth};
use bencher_rate_limiter::BandwidthLimiter;
use bencher_rate_limiter::snapshot::BandwidthSnapshot;
use dropshot::HttpError;

use crate::{
    context::RateLimitingError, error::too_many_requests, model::organization::QueryOrganization,
};

const DAY: Duration = Duration::from_secs(60 * 60 * 24);

const DEFAULT_UNCLAIMED_BANDWIDTH: u64 = 1 << 30;
const DEFAULT_FREE_BANDWIDTH: u64 = 10 << 30;
const DEFAULT_PLUS_BANDWIDTH: u64 = 100 << 30;

const BYTES_PER_GIB: u64 = 1 << 30;

pub(super) struct BandwidthRateLimiter {
    limiter: BandwidthLimiter<OrganizationUuid>,
    unclaimed_limit: u64,
    free_limit: u64,
    plus_limit: u64,
}

impl Default for BandwidthRateLimiter {
    fn default() -> Self {
        Self {
            limiter: BandwidthLimiter::new(DAY),
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
            limiter: BandwidthLimiter::new(DAY),
            unclaimed_limit: unclaimed.unwrap_or(DEFAULT_UNCLAIMED_BANDWIDTH),
            free_limit: free.unwrap_or(DEFAULT_FREE_BANDWIDTH),
            plus_limit: plus.unwrap_or(DEFAULT_PLUS_BANDWIDTH),
        }
    }
}

impl BandwidthRateLimiter {
    pub fn max() -> Self {
        Self {
            limiter: BandwidthLimiter::new(DAY),
            unclaimed_limit: u64::MAX,
            free_limit: u64::MAX,
            plus_limit: u64::MAX,
        }
    }

    pub fn snapshot(&self) -> BandwidthSnapshot<OrganizationUuid> {
        self.limiter.snapshot()
    }

    pub fn restore(&self, snapshot: BandwidthSnapshot<OrganizationUuid>) {
        self.limiter.restore(snapshot);
    }

    pub fn prune(&self) {
        self.limiter.prune();
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

        if self.limiter.check_at(&org_uuid, limit, now) {
            Ok(())
        } else {
            Err(too_many_requests(RateLimitingError::OciBandwidth {
                organization: organization.clone(),
                limit_gib: limit.saturating_div(BYTES_PER_GIB),
            }))
        }
    }

    pub fn record(&self, org_uuid: OrganizationUuid, bytes: u64) {
        self.limiter.record(org_uuid, bytes);
    }

    #[cfg(test)]
    fn record_at(&self, org_uuid: OrganizationUuid, bytes: u64, now: SystemTime) {
        self.limiter.record_at(org_uuid, bytes, now);
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
            limiter: BandwidthLimiter::new(DAY),
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
            limiter: BandwidthLimiter::new(DAY),
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
            limiter: BandwidthLimiter::new(DAY),
            unclaimed_limit: 100,
            free_limit: 1000,
            plus_limit: 10_000,
        };

        let now = test_now();
        let org_uuid = org_uuid();
        let org = org();

        let old = now - Duration::from_secs(25 * 60 * 60);
        limiter.record_at(org_uuid, 500, old);

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
        let snapshot = limiter.snapshot();
        assert!(snapshot.events.is_empty() || !snapshot.events.contains_key(&org_uuid));
    }
}
