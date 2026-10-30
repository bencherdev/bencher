use std::net::IpAddr;

use bencher_json::{OrganizationUuid, ProjectUuid, RunnerUuid, UserUuid};
use bencher_rate_limiter::snapshot::{
    BandwidthSnapshot, RateLimiterSnapshot as GenericRateLimiterSnapshot,
};
use serde::{Deserialize, Serialize};

use super::EmailKey;

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct RateLimitingSnapshot {
    pub public: PublicRateLimiterSnapshot,
    pub user: UserRateLimiterSnapshot,
    pub project: ProjectRateLimiterSnapshot,
    pub runner: RunnerRateLimiterSnapshot,
    pub bandwidth: BandwidthSnapshot<OrganizationUuid>,
    /// Missing from a snapshot saved before the limit existed.
    #[serde(default)]
    pub email: GenericRateLimiterSnapshot<EmailKey>,
}

impl RateLimitingSnapshot {
    pub fn new(
        public: PublicRateLimiterSnapshot,
        user: UserRateLimiterSnapshot,
        project: ProjectRateLimiterSnapshot,
        runner: RunnerRateLimiterSnapshot,
        bandwidth: BandwidthSnapshot<OrganizationUuid>,
        email: GenericRateLimiterSnapshot<EmailKey>,
    ) -> Self {
        Self {
            public,
            user,
            project,
            runner,
            bandwidth,
            email,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct PublicRateLimiterSnapshot {
    pub requests: GenericRateLimiterSnapshot<IpAddr>,
    pub attempts: GenericRateLimiterSnapshot<IpAddr>,
    pub runs: GenericRateLimiterSnapshot<IpAddr>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct UserRateLimiterSnapshot {
    pub requests: GenericRateLimiterSnapshot<UserUuid>,
    pub attempts: GenericRateLimiterSnapshot<UserUuid>,
    pub credentials: GenericRateLimiterSnapshot<UserUuid>,
    pub organizations: GenericRateLimiterSnapshot<UserUuid>,
    pub invites: GenericRateLimiterSnapshot<UserUuid>,
    pub runs: GenericRateLimiterSnapshot<UserUuid>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct ProjectRateLimiterSnapshot {
    pub requests: GenericRateLimiterSnapshot<ProjectUuid>,
    pub runs: GenericRateLimiterSnapshot<ProjectUuid>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct RunnerRateLimiterSnapshot {
    pub requests: GenericRateLimiterSnapshot<RunnerUuid>,
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, hash::Hash, time::SystemTime};

    use bencher_rate_limiter::snapshot::WindowSnapshot;

    use super::*;

    fn empty_window_snapshot<K: Eq + Hash>() -> WindowSnapshot<K> {
        WindowSnapshot {
            events: HashMap::new(),
        }
    }

    fn empty_limiter_snapshot<K: Eq + Hash>() -> GenericRateLimiterSnapshot<K> {
        GenericRateLimiterSnapshot {
            minute: empty_window_snapshot(),
            hour: empty_window_snapshot(),
            day: empty_window_snapshot(),
        }
    }

    #[test]
    fn round_trip_json() {
        let now = bencher_rate_limiter::epoch_bucket(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            60,
        );
        let ip: IpAddr = "192.168.1.1".parse().unwrap();

        let snapshot = RateLimitingSnapshot {
            public: PublicRateLimiterSnapshot {
                requests: GenericRateLimiterSnapshot {
                    minute: WindowSnapshot {
                        events: HashMap::from([(ip, vec![(now, 2), (now - 1, 1)])]),
                    },
                    hour: empty_window_snapshot(),
                    day: empty_window_snapshot(),
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
            bandwidth: BandwidthSnapshot {
                events: HashMap::new(),
            },
            email: empty_limiter_snapshot(),
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

        let snapshot = RateLimitingSnapshot {
            public: PublicRateLimiterSnapshot {
                requests: GenericRateLimiterSnapshot {
                    minute: WindowSnapshot {
                        events: HashMap::from([(
                            ip,
                            vec![(bencher_rate_limiter::epoch_bucket(old_secs, 60), 1)],
                        )]),
                    },
                    hour: WindowSnapshot {
                        events: HashMap::from([(
                            ip,
                            vec![(bencher_rate_limiter::epoch_bucket(old_secs, 3600), 1)],
                        )]),
                    },
                    day: empty_window_snapshot(),
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
            bandwidth: BandwidthSnapshot {
                events: HashMap::new(),
            },
            email: empty_limiter_snapshot(),
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

    /// Every window over any limit, indexed in buckets the size of the whole window.
    fn window_sized_limiter_snapshot<K: Eq + Hash + Copy>(
        key: K,
        now_secs: u64,
    ) -> GenericRateLimiterSnapshot<K> {
        let window = |window: std::time::Duration| WindowSnapshot {
            events: HashMap::from([(
                key,
                vec![(
                    bencher_rate_limiter::epoch_bucket(now_secs, window.as_secs()),
                    u32::MAX,
                )],
            )]),
        };
        GenericRateLimiterSnapshot {
            minute: window(bencher_rate_limiter::MINUTE),
            hour: window(bencher_rate_limiter::HOUR),
            day: window(bencher_rate_limiter::DAY),
        }
    }

    #[test]
    fn load_drops_window_sized_buckets() {
        use crate::context::RateLimiting;

        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("bencher.db");
        let snapshot_path = dir.path().join("rate_limiting.json");
        let log = slog::Logger::root(slog::Discard, slog::o!());

        let now_secs = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let ip: IpAddr = "10.0.0.1".parse().unwrap();
        let user = UserUuid::from(uuid::Uuid::nil());
        let project = ProjectUuid::from(uuid::Uuid::nil());
        let runner = RunnerUuid::from(uuid::Uuid::nil());
        let organization = OrganizationUuid::from(uuid::Uuid::nil());
        let address: bencher_json::Email = "ada@example.com".parse().unwrap();
        let email = EmailKey::from(&address);

        let snapshot = RateLimitingSnapshot {
            public: PublicRateLimiterSnapshot {
                requests: window_sized_limiter_snapshot(ip, now_secs),
                attempts: window_sized_limiter_snapshot(ip, now_secs),
                runs: window_sized_limiter_snapshot(ip, now_secs),
            },
            user: UserRateLimiterSnapshot {
                requests: window_sized_limiter_snapshot(user, now_secs),
                attempts: window_sized_limiter_snapshot(user, now_secs),
                credentials: window_sized_limiter_snapshot(user, now_secs),
                organizations: window_sized_limiter_snapshot(user, now_secs),
                invites: window_sized_limiter_snapshot(user, now_secs),
                runs: window_sized_limiter_snapshot(user, now_secs),
            },
            project: ProjectRateLimiterSnapshot {
                requests: window_sized_limiter_snapshot(project, now_secs),
                runs: window_sized_limiter_snapshot(project, now_secs),
            },
            runner: RunnerRateLimiterSnapshot {
                requests: window_sized_limiter_snapshot(runner, now_secs),
            },
            bandwidth: BandwidthSnapshot {
                events: HashMap::from([(
                    organization,
                    vec![(
                        bencher_rate_limiter::epoch_bucket(
                            now_secs,
                            bencher_rate_limiter::DAY.as_secs(),
                        ),
                        u64::MAX,
                    )],
                )]),
            },
            email: window_sized_limiter_snapshot(email, now_secs),
        };
        std::fs::write(&snapshot_path, serde_json::to_string(&snapshot).unwrap()).unwrap();

        let limiter = RateLimiting::default();
        limiter.load(&db_path, &log).unwrap();

        limiter.save(&db_path, &log).unwrap();
        let empty = RateLimitingSnapshot {
            public: PublicRateLimiterSnapshot {
                requests: empty_limiter_snapshot(),
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
            bandwidth: BandwidthSnapshot {
                events: HashMap::new(),
            },
            email: empty_limiter_snapshot(),
        };
        let saved: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&snapshot_path).unwrap()).unwrap();
        assert_eq!(saved, serde_json::to_value(&empty).unwrap());

        limiter.public_request(ip).unwrap();
        limiter.public_auth_attempt(ip).unwrap();
        limiter.unclaimed_run(ip).unwrap();
        limiter.user_request(user).unwrap();
        limiter.auth_attempt(user).unwrap();
        limiter.create_credential(user).unwrap();
        limiter.create_organization(user).unwrap();
        limiter.user_invite(user).unwrap();
        limiter.claimed_run(user).unwrap();
        limiter.project_request(project).unwrap();
        limiter.project_run(project).unwrap();
        limiter.runner_request(runner).unwrap();
        limiter.sign_in_email(&address).unwrap();
    }

    // The snapshot keeps an address at its cap, keyed so that the file holds no address.
    #[test]
    fn email_cap_survives_a_restart_without_the_address() {
        use crate::context::RateLimiting;

        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("bencher.db");
        let log = slog::Logger::root(slog::Discard, slog::o!());
        let address: bencher_json::Email = "ada.lovelace@example.com".parse().unwrap();

        let limiter = RateLimiting::default();
        limiter.sign_in_email(&address).unwrap();
        limiter.sign_in_email(&address).unwrap();
        limiter.sign_in_email(&address).unwrap_err();
        limiter.save(&db_path, &log).unwrap();

        let json = std::fs::read_to_string(dir.path().join("rate_limiting.json")).unwrap();
        assert!(!json.contains("ada.lovelace"), "{json}");
        assert!(!json.contains("example.com"), "{json}");
        let snapshot: serde_json::Value = serde_json::from_str(&json).unwrap();
        let keys = snapshot["email"]["minute"]["events"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        let sha256 = {
            use sha2::Digest as _;
            hex::encode(sha2::Sha256::digest(b"ada.lovelace@example.com"))
        };
        assert_eq!(keys, [sha256]);

        let restored = RateLimiting::default();
        restored.load(&db_path, &log).unwrap();
        restored.sign_in_email(&address).unwrap_err();
        let other: bencher_json::Email = "grace@example.com".parse().unwrap();
        restored.sign_in_email(&other).unwrap();
    }

    // A snapshot saved before the per-address limit existed still loads.
    #[test]
    fn load_a_snapshot_without_the_email_limit() {
        use crate::context::RateLimiting;

        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("bencher.db");
        let log = slog::Logger::root(slog::Discard, slog::o!());
        let ip: IpAddr = "10.0.0.1".parse().unwrap();

        let limiter = RateLimiting::default();
        limiter.public_request(ip).unwrap();
        limiter.save(&db_path, &log).unwrap();
        let snapshot_path = dir.path().join("rate_limiting.json");
        let mut json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&snapshot_path).unwrap()).unwrap();
        json.as_object_mut().unwrap().remove("email").unwrap();
        std::fs::write(&snapshot_path, json.to_string()).unwrap();

        let restored = RateLimiting::default();
        restored.load(&db_path, &log).unwrap();
        restored.save(&db_path, &log).unwrap();
        let snap: RateLimitingSnapshot =
            serde_json::from_str(&std::fs::read_to_string(&snapshot_path).unwrap()).unwrap();
        assert!(snap.public.requests.minute.events.contains_key(&ip));
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
        limiter.load(&db_path, &log).unwrap_err();
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

    #[test]
    fn load_legacy_format_errs() {
        use crate::context::RateLimiting;

        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("bencher.db");
        let snapshot_path = dir.path().join("rate_limiting.json");

        // Regression for the production failure: a snapshot written by the pre-bucketing format stored
        // raw second timestamps in a flat array, but the current code expects `Vec<(EpochBucket, u32)>`
        // (arrays of `[bucket, count]`). Deserializing `[1778809249, 1]` as a list of tuples yields:
        // "invalid type: integer `1778809249`, expected a tuple of size 2".
        let legacy = r#"{"public":{"requests":{"minute":{"events":{"1.2.3.4":[1778809249,1]}}}}}"#;
        std::fs::write(&snapshot_path, legacy).unwrap();

        let log = slog::Logger::root(slog::Discard, slog::o!());
        let limiter = RateLimiting::default();
        let err = limiter.load(&db_path, &log).unwrap_err();

        assert!(matches!(
            err,
            super::super::RateLimitingPersistError::Deserialize(_)
        ));
        // The bad file is left in place; the next successful save overwrites it.
        assert!(snapshot_path.exists());
    }
}
