use std::{collections::HashMap, hash::Hash, net::IpAddr};

use bencher_json::{OrganizationUuid, ProjectUuid, RunnerUuid, UserUuid};
use serde::{Deserialize, Serialize};

pub(super) type EpochBucket = u64;

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct RateLimitingSnapshot {
    pub public: PublicRateLimiterSnapshot,
    pub user: UserRateLimiterSnapshot,
    pub project: ProjectRateLimiterSnapshot,
    pub runner: RunnerRateLimiterSnapshot,
    pub bandwidth: BandwidthRateLimiterSnapshot,
}

impl RateLimitingSnapshot {
    pub fn new(
        public: PublicRateLimiterSnapshot,
        user: UserRateLimiterSnapshot,
        project: ProjectRateLimiterSnapshot,
        runner: RunnerRateLimiterSnapshot,
        bandwidth: BandwidthRateLimiterSnapshot,
    ) -> Self {
        Self {
            public,
            user,
            project,
            runner,
            bandwidth,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct RateLimiterInnerSnapshot<K: Eq + Hash> {
    pub events: HashMap<K, Vec<(EpochBucket, u32)>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct RateLimiterSnapshot<K: Eq + Hash> {
    pub minute: RateLimiterInnerSnapshot<K>,
    pub hour: RateLimiterInnerSnapshot<K>,
    pub day: RateLimiterInnerSnapshot<K>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct PublicRateLimiterSnapshot {
    pub requests: RateLimiterSnapshot<IpAddr>,
    pub attempts: RateLimiterSnapshot<IpAddr>,
    pub runs: RateLimiterSnapshot<IpAddr>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct UserRateLimiterSnapshot {
    pub requests: RateLimiterSnapshot<UserUuid>,
    pub attempts: RateLimiterSnapshot<UserUuid>,
    pub credentials: RateLimiterSnapshot<UserUuid>,
    pub organizations: RateLimiterSnapshot<UserUuid>,
    pub invites: RateLimiterSnapshot<UserUuid>,
    pub runs: RateLimiterSnapshot<UserUuid>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct ProjectRateLimiterSnapshot {
    pub requests: RateLimiterSnapshot<ProjectUuid>,
    pub runs: RateLimiterSnapshot<ProjectUuid>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct RunnerRateLimiterSnapshot {
    pub requests: RateLimiterSnapshot<RunnerUuid>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct BandwidthRateLimiterSnapshot {
    pub events: HashMap<OrganizationUuid, Vec<(EpochBucket, u64)>>,
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use super::*;

    fn now_epoch_bucket() -> EpochBucket {
        use super::super::epoch_bucket;
        epoch_bucket(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            60,
        )
    }

    fn empty_inner_snapshot<K: Eq + Hash>() -> RateLimiterInnerSnapshot<K> {
        RateLimiterInnerSnapshot {
            events: HashMap::new(),
        }
    }

    fn empty_limiter_snapshot<K: Eq + Hash>() -> RateLimiterSnapshot<K> {
        RateLimiterSnapshot {
            minute: empty_inner_snapshot(),
            hour: empty_inner_snapshot(),
            day: empty_inner_snapshot(),
        }
    }

    #[test]
    fn round_trip_json() {
        let now = now_epoch_bucket();
        let ip: IpAddr = "192.168.1.1".parse().unwrap();
        let mut events = HashMap::new();
        events.insert(ip, vec![(now, 2), (now - 1, 1)]);

        let snapshot = RateLimitingSnapshot {
            public: PublicRateLimiterSnapshot {
                requests: RateLimiterSnapshot {
                    minute: RateLimiterInnerSnapshot {
                        events: events.clone(),
                    },
                    hour: empty_inner_snapshot(),
                    day: empty_inner_snapshot(),
                },
                attempts: empty_limiter_snapshot(),
                runs: empty_limiter_snapshot(),
            },
            user: UserRateLimiterSnapshot {
                requests: empty_limiter_snapshot(),
                attempts: empty_limiter_snapshot(),
                credentials: empty_limiter_snapshot(),
                organizations: empty_limiter_snapshot(),
                invites: empty_limiter_snapshot(),
                runs: empty_limiter_snapshot(),
            },
            project: ProjectRateLimiterSnapshot {
                requests: empty_limiter_snapshot(),
                runs: empty_limiter_snapshot(),
            },
            runner: RunnerRateLimiterSnapshot {
                requests: empty_limiter_snapshot(),
            },
            bandwidth: BandwidthRateLimiterSnapshot {
                events: HashMap::new(),
            },
        };

        let json = serde_json::to_string(&snapshot).unwrap();
        let restored: RateLimitingSnapshot = serde_json::from_str(&json).unwrap();

        let restored_events = &restored.public.requests.minute.events;
        assert_eq!(
            restored_events.get(&ip).unwrap(),
            &vec![(now, 2), (now - 1, 1)]
        );
    }

    #[test]
    fn save_load_round_trip() {
        use crate::context::RateLimiting;

        let limiter = RateLimiting::default();
        let ip: IpAddr = "10.0.0.1".parse().unwrap();
        limiter.public_request(ip).unwrap();

        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("bencher.db");
        let log = slog::Logger::root(slog::Discard, slog::o!());

        limiter.save(&db_path, &log).unwrap();

        let snapshot_path = db_path.parent().unwrap().join("rate_limiting.json");
        assert!(snapshot_path.exists());

        let limiter2 = RateLimiting::default();
        limiter2.load(&db_path, &log).unwrap();

        assert!(snapshot_path.exists());

        limiter2.save(&db_path, &log).unwrap();
        let json = std::fs::read_to_string(&snapshot_path).unwrap();
        let snap: RateLimitingSnapshot = serde_json::from_str(&json).unwrap();
        assert!(snap.public.requests.minute.events.contains_key(&ip));
    }

    #[test]
    fn save_load_expired_entries_filtered() {
        use super::super::epoch_bucket;
        use crate::context::RateLimiting;

        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("bencher.db");
        let snapshot_path = dir.path().join("rate_limiting.json");
        let log = slog::Logger::root(slog::Discard, slog::o!());

        let now_secs = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let old_secs = now_secs - 7200;
        let ip: IpAddr = "10.0.0.1".parse().unwrap();

        let mut minute_events = HashMap::new();
        minute_events.insert(ip, vec![(epoch_bucket(old_secs, 60), 1)]);
        let mut hour_events = HashMap::new();
        hour_events.insert(ip, vec![(epoch_bucket(old_secs, 3600), 1)]);

        let snapshot = RateLimitingSnapshot {
            public: PublicRateLimiterSnapshot {
                requests: RateLimiterSnapshot {
                    minute: RateLimiterInnerSnapshot {
                        events: minute_events,
                    },
                    hour: RateLimiterInnerSnapshot {
                        events: hour_events,
                    },
                    day: empty_inner_snapshot(),
                },
                attempts: empty_limiter_snapshot(),
                runs: empty_limiter_snapshot(),
            },
            user: UserRateLimiterSnapshot {
                requests: empty_limiter_snapshot(),
                attempts: empty_limiter_snapshot(),
                credentials: empty_limiter_snapshot(),
                organizations: empty_limiter_snapshot(),
                invites: empty_limiter_snapshot(),
                runs: empty_limiter_snapshot(),
            },
            project: ProjectRateLimiterSnapshot {
                requests: empty_limiter_snapshot(),
                runs: empty_limiter_snapshot(),
            },
            runner: RunnerRateLimiterSnapshot {
                requests: empty_limiter_snapshot(),
            },
            bandwidth: BandwidthRateLimiterSnapshot {
                events: HashMap::new(),
            },
        };
        let json = serde_json::to_string(&snapshot).unwrap();
        std::fs::write(&snapshot_path, json).unwrap();

        let limiter = RateLimiting::default();
        limiter.load(&db_path, &log).unwrap();

        limiter.save(&db_path, &log).unwrap();
        let json = std::fs::read_to_string(&snapshot_path).unwrap();
        let snap: RateLimitingSnapshot = serde_json::from_str(&json).unwrap();
        assert!(snap.public.requests.minute.events.is_empty());
        assert!(snap.public.requests.hour.events.is_empty());
    }

    #[test]
    fn load_missing_file() {
        use crate::context::RateLimiting;

        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("bencher.db");
        let log = slog::Logger::root(slog::Discard, slog::o!());

        let limiter = RateLimiting::default();
        limiter.load(&db_path, &log).unwrap();
    }

    #[test]
    fn load_corrupt_file() {
        use crate::context::RateLimiting;

        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("bencher.db");
        let snapshot_path = dir.path().join("rate_limiting.json");
        std::fs::write(&snapshot_path, "not valid json").unwrap();

        let log = slog::Logger::root(slog::Discard, slog::o!());
        let limiter = RateLimiting::default();
        assert!(limiter.load(&db_path, &log).is_err());
        assert!(snapshot_path.exists());
    }

    #[test]
    fn bandwidth_snapshot_round_trip() {
        use crate::context::RateLimiting;

        let limiter = RateLimiting::default();
        let org_uuid = OrganizationUuid::from(uuid::Uuid::nil());
        limiter.record_oci_bandwidth(org_uuid, 1024);
        limiter.record_oci_bandwidth(org_uuid, 2048);

        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("bencher.db");
        let log = slog::Logger::root(slog::Discard, slog::o!());

        limiter.save(&db_path, &log).unwrap();

        let snapshot_path = dir.path().join("rate_limiting.json");
        let json = std::fs::read_to_string(&snapshot_path).unwrap();
        let snap: RateLimitingSnapshot = serde_json::from_str(&json).unwrap();

        let entries = snap
            .bandwidth
            .events
            .get(&org_uuid)
            .expect("org_uuid present");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries.first().expect("first entry").1, 3072);

        let limiter2 = RateLimiting::default();
        limiter2.load(&db_path, &log).unwrap();

        limiter2.save(&db_path, &log).unwrap();
        let json = std::fs::read_to_string(&snapshot_path).unwrap();
        let snap: RateLimitingSnapshot = serde_json::from_str(&json).unwrap();
        let entries = snap
            .bandwidth
            .events
            .get(&org_uuid)
            .expect("org_uuid present");
        assert_eq!(entries.len(), 1);
    }
}
