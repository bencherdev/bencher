//! Resolved replication configuration: JSON config plus battle-tested
//! defaults (matching Litestream where applicable).

use std::str::FromStr as _;
use std::time::Duration;

use bencher_json::system::config::{JsonReplication, ReplicationTarget};
use camino::Utf8PathBuf;

use crate::local::LocalStorage;
use crate::s3::S3Storage;
use crate::storage::ReplicaStorage;

/// How often to ship new WAL frames.
pub const DEFAULT_SYNC_INTERVAL: Duration = Duration::from_secs(1);
/// Minimum interval between checkpoints.
pub const DEFAULT_CHECKPOINT_INTERVAL: Duration = Duration::from_mins(1);
/// Minimum WAL pages before a checkpoint triggers.
pub const DEFAULT_MIN_CHECKPOINT_PAGES: u32 = 1000;
/// How often to start a new generation with a fresh snapshot.
pub const DEFAULT_SNAPSHOT_INTERVAL: Duration = Duration::from_hours(24);
/// Snapshot copy throttle in MiB per second.
pub const DEFAULT_SNAPSHOT_THROTTLE_MIB: u32 = 32;
/// Number of generations to retain.
pub const DEFAULT_RETENTION_GENERATIONS: u32 = 3;
/// How often to run restore-and-compare verification. On by default: it is
/// the backstop for the residual failure modes no local evidence can catch
/// (an external writer churning the database while the replicator is down).
pub const DEFAULT_VERIFICATION_INTERVAL: Duration = Duration::from_hours(24);
/// Deadline for the final WAL ship at shutdown: fits inside Fly's 6 second
/// kill timeout with margin for the connection drain in `server.close()`.
/// This bounds ONLY the ship loop; on a complete drain in sole mode a final
/// checkpoint runs AFTER it (unbounded, so it can exceed the kill window). The
/// checkpoint is crash-safe if truncated: it only backfills already-shipped
/// frames (invariant I1), so an interrupted checkpoint costs a re-snapshot on
/// the next boot, never data.
pub const DEFAULT_SHUTDOWN_SYNC_TIMEOUT: Duration = Duration::from_secs(4);

#[derive(Debug, thiserror::Error)]
pub enum ReplicaConfigError {
    #[error("Replica file target path is not valid UTF-8: {0}")]
    NonUtf8Path(std::path::PathBuf),
    #[error(
        "checkpoint_interval_secs must be greater than 0 (0 would attempt a checkpoint every tick)"
    )]
    ZeroCheckpointInterval,
    #[error(
        "snapshot_interval_secs must be greater than 0 (0 would start a new generation every tick)"
    )]
    ZeroSnapshotInterval,
    #[error("Replica S3 bucket must not be empty")]
    EmptyS3Bucket,
    #[error("Replica S3 access_key_id must not be empty")]
    EmptyS3AccessKeyId,
    #[error("Replica S3 secret_access_key must not be empty")]
    EmptyS3SecretAccessKey,
    #[error("Replica S3 endpoint is not a valid URL: {0}")]
    InvalidS3Endpoint(bencher_json::ValidError),
}

/// Resolved configuration for the replicator.
#[derive(Debug, Clone)]
pub struct ReplicaConfig {
    pub target: ReplicationTarget,
    pub sync_interval: Duration,
    pub checkpoint_interval: Duration,
    pub min_checkpoint_pages: u32,
    pub snapshot_interval: Duration,
    pub snapshot_throttle_mib: u32,
    pub retention_generations: u32,
    /// Restore-and-compare verification interval; `None` disables it
    /// (config value 0 opts out; absent means the default).
    pub verification_interval: Option<Duration>,
    pub shutdown_sync_timeout: Duration,
    /// Largest single WAL transaction (raw bytes) the ship path will accept.
    /// A transaction beyond this is structurally unshippable (it would exceed
    /// the restore decompression bound and poison every restore), so shipping
    /// it fails loudly. Not operator-configurable: it defaults to
    /// [`crate::segment::MAX_DECOMPRESSED_BYTES`] and must never exceed it
    /// (tests lower it to exercise the poison path).
    pub max_transaction_bytes: u64,
}

impl TryFrom<JsonReplication> for ReplicaConfig {
    type Error = ReplicaConfigError;

    fn try_from(json: JsonReplication) -> Result<Self, Self::Error> {
        let JsonReplication {
            target,
            sync_interval_secs,
            checkpoint_interval_secs,
            min_checkpoint_pages,
            snapshot_interval_secs,
            snapshot_throttle_mib,
            retention_generations,
            verification_interval_secs,
            shutdown_sync_timeout_secs,
        } = json;
        // Validate the target up front, at config load time, so a
        // misconfiguration fails loudly here instead of as an infinite runtime
        // retry loop.
        match &target {
            ReplicationTarget::File { path } => {
                if Utf8PathBuf::from_path_buf(path.clone()).is_err() {
                    return Err(ReplicaConfigError::NonUtf8Path(path.clone()));
                }
            },
            ReplicationTarget::S3 {
                bucket,
                endpoint,
                access_key_id,
                secret_access_key,
                ..
            } => {
                if bucket.trim().is_empty() {
                    return Err(ReplicaConfigError::EmptyS3Bucket);
                }
                if access_key_id.trim().is_empty() {
                    return Err(ReplicaConfigError::EmptyS3AccessKeyId);
                }
                // `Secret` deserialization rejects the fully-empty string but
                // not a whitespace-only one; catch that here like the fields
                // above.
                if secret_access_key.as_ref().trim().is_empty() {
                    return Err(ReplicaConfigError::EmptyS3SecretAccessKey);
                }
                if let Some(endpoint) = endpoint {
                    bencher_json::Url::from_str(endpoint)
                        .map_err(ReplicaConfigError::InvalidS3Endpoint)?;
                }
            },
        }
        // A zero checkpoint or snapshot interval makes the corresponding
        // due-ness check true every tick (a checkpoint attempt, or a whole new
        // generation, every second). Retention clamps to 1 just below;
        // verification 0 legitimately opts out; a zero sync interval is clamped
        // to 1 second at runtime in the replicator tick shell (it drives the
        // loop period, not a due-ness check, so it cannot be validated to a
        // resolved `Duration` here).
        if checkpoint_interval_secs == Some(0) {
            return Err(ReplicaConfigError::ZeroCheckpointInterval);
        }
        if snapshot_interval_secs == Some(0) {
            return Err(ReplicaConfigError::ZeroSnapshotInterval);
        }
        Ok(Self {
            target,
            sync_interval: sync_interval_secs.map_or(DEFAULT_SYNC_INTERVAL, |secs| {
                Duration::from_secs(secs.into())
            }),
            checkpoint_interval: checkpoint_interval_secs
                .map_or(DEFAULT_CHECKPOINT_INTERVAL, |secs| {
                    Duration::from_secs(secs.into())
                }),
            min_checkpoint_pages: min_checkpoint_pages.unwrap_or(DEFAULT_MIN_CHECKPOINT_PAGES),
            snapshot_interval: snapshot_interval_secs.map_or(DEFAULT_SNAPSHOT_INTERVAL, |secs| {
                Duration::from_secs(secs.into())
            }),
            snapshot_throttle_mib: snapshot_throttle_mib.unwrap_or(DEFAULT_SNAPSHOT_THROTTLE_MIB),
            retention_generations: retention_generations
                .unwrap_or(DEFAULT_RETENTION_GENERATIONS)
                .max(1),
            verification_interval: match verification_interval_secs {
                // Explicit zero opts out of verification entirely.
                Some(0) => None,
                Some(secs) => Some(Duration::from_secs(secs.into())),
                None => Some(DEFAULT_VERIFICATION_INTERVAL),
            },
            shutdown_sync_timeout: shutdown_sync_timeout_secs
                .map_or(DEFAULT_SHUTDOWN_SYNC_TIMEOUT, |secs| {
                    Duration::from_secs(secs.into())
                }),
            max_transaction_bytes: crate::segment::MAX_DECOMPRESSED_BYTES,
        })
    }
}

impl ReplicaConfig {
    /// Build the storage backend for the configured target.
    ///
    /// The `File` path was validated as UTF-8 in `try_from`, so this cannot
    /// fail afterward; a race is impossible because the target is immutable.
    #[must_use]
    pub fn build_storage(&self) -> ReplicaStorage {
        match &self.target {
            ReplicationTarget::File { path } => {
                let root = Utf8PathBuf::from_path_buf(path.clone())
                    .unwrap_or_else(|path| Utf8PathBuf::from(path.to_string_lossy().into_owned()));
                ReplicaStorage::Local(LocalStorage::new(root))
            },
            ReplicationTarget::S3 {
                bucket,
                path,
                endpoint,
                region,
                access_key_id,
                secret_access_key,
            } => ReplicaStorage::S3(Box::new(S3Storage::new(
                bucket.clone(),
                path.clone(),
                endpoint.clone(),
                region.clone(),
                access_key_id.clone(),
                secret_access_key.as_ref(),
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use bencher_json::system::config::{JsonReplication, ReplicationTarget};
    use pretty_assertions::assert_eq;

    use super::{
        DEFAULT_CHECKPOINT_INTERVAL, DEFAULT_MIN_CHECKPOINT_PAGES, DEFAULT_RETENTION_GENERATIONS,
        DEFAULT_SHUTDOWN_SYNC_TIMEOUT, DEFAULT_SNAPSHOT_INTERVAL, DEFAULT_SNAPSHOT_THROTTLE_MIB,
        DEFAULT_SYNC_INTERVAL, DEFAULT_VERIFICATION_INTERVAL, ReplicaConfig,
    };

    fn file_json(overrides: impl FnOnce(&mut JsonReplication)) -> JsonReplication {
        let mut json = JsonReplication {
            target: ReplicationTarget::File {
                path: PathBuf::from("/tmp/replica"),
            },
            sync_interval_secs: None,
            checkpoint_interval_secs: None,
            min_checkpoint_pages: None,
            snapshot_interval_secs: None,
            snapshot_throttle_mib: None,
            retention_generations: None,
            verification_interval_secs: None,
            shutdown_sync_timeout_secs: None,
        };
        overrides(&mut json);
        json
    }

    fn s3_json(bucket: &str, access_key_id: &str, endpoint: Option<&str>) -> JsonReplication {
        JsonReplication {
            target: ReplicationTarget::S3 {
                bucket: bucket.to_owned(),
                path: None,
                endpoint: endpoint.map(str::to_owned),
                region: None,
                access_key_id: access_key_id.to_owned(),
                secret_access_key: "secret".parse().expect("valid secret"),
            },
            sync_interval_secs: None,
            checkpoint_interval_secs: None,
            min_checkpoint_pages: None,
            snapshot_interval_secs: None,
            snapshot_throttle_mib: None,
            retention_generations: None,
            verification_interval_secs: None,
            shutdown_sync_timeout_secs: None,
        }
    }

    #[test]
    fn defaults_applied() {
        let config = ReplicaConfig::try_from(file_json(|_| {})).unwrap();
        assert_eq!(config.sync_interval, DEFAULT_SYNC_INTERVAL);
        assert_eq!(config.checkpoint_interval, DEFAULT_CHECKPOINT_INTERVAL);
        assert_eq!(config.min_checkpoint_pages, DEFAULT_MIN_CHECKPOINT_PAGES);
        assert_eq!(config.snapshot_interval, DEFAULT_SNAPSHOT_INTERVAL);
        assert_eq!(config.snapshot_throttle_mib, DEFAULT_SNAPSHOT_THROTTLE_MIB);
        assert_eq!(config.retention_generations, DEFAULT_RETENTION_GENERATIONS);
        assert_eq!(
            config.verification_interval,
            Some(DEFAULT_VERIFICATION_INTERVAL)
        );
        assert_eq!(config.shutdown_sync_timeout, DEFAULT_SHUTDOWN_SYNC_TIMEOUT);
    }

    #[test]
    fn overrides_applied() {
        let config = ReplicaConfig::try_from(file_json(|json| {
            json.sync_interval_secs = Some(2);
            json.checkpoint_interval_secs = Some(120);
            json.min_checkpoint_pages = Some(500);
            json.snapshot_interval_secs = Some(3600);
            json.snapshot_throttle_mib = Some(64);
            json.retention_generations = Some(5);
            json.verification_interval_secs = Some(86_400);
            json.shutdown_sync_timeout_secs = Some(2);
        }))
        .unwrap();
        assert_eq!(config.sync_interval, Duration::from_secs(2));
        assert_eq!(config.checkpoint_interval, Duration::from_mins(2));
        assert_eq!(config.min_checkpoint_pages, 500);
        assert_eq!(config.snapshot_interval, Duration::from_hours(1));
        assert_eq!(config.snapshot_throttle_mib, 64);
        assert_eq!(config.retention_generations, 5);
        assert_eq!(config.verification_interval, Some(Duration::from_hours(24)));
        assert_eq!(config.shutdown_sync_timeout, Duration::from_secs(2));
    }

    #[test]
    fn retention_zero_clamps_to_one() {
        let config = ReplicaConfig::try_from(file_json(|json| {
            json.retention_generations = Some(0);
        }))
        .unwrap();
        assert_eq!(config.retention_generations, 1);
    }

    #[test]
    fn verification_zero_opts_out() {
        let config = ReplicaConfig::try_from(file_json(|json| {
            json.verification_interval_secs = Some(0);
        }))
        .unwrap();
        assert_eq!(config.verification_interval, None);
    }

    #[test]
    fn checkpoint_interval_zero_rejected() {
        let error = ReplicaConfig::try_from(file_json(|json| {
            json.checkpoint_interval_secs = Some(0);
        }))
        .unwrap_err();
        assert!(
            matches!(error, super::ReplicaConfigError::ZeroCheckpointInterval),
            "expected ZeroCheckpointInterval, got {error:?}"
        );
    }

    #[test]
    fn snapshot_interval_zero_rejected() {
        let error = ReplicaConfig::try_from(file_json(|json| {
            json.snapshot_interval_secs = Some(0);
        }))
        .unwrap_err();
        assert!(
            matches!(error, super::ReplicaConfigError::ZeroSnapshotInterval),
            "expected ZeroSnapshotInterval, got {error:?}"
        );
    }

    #[test]
    fn max_transaction_bytes_defaults_to_decompression_bound() {
        let config = ReplicaConfig::try_from(file_json(|_| {})).unwrap();
        assert_eq!(
            config.max_transaction_bytes,
            crate::segment::MAX_DECOMPRESSED_BYTES
        );
    }

    #[test]
    fn s3_valid_target_accepted() {
        let config =
            ReplicaConfig::try_from(s3_json("bucket", "AKIA", Some("https://r2.example.com")))
                .unwrap();
        assert!(matches!(config.target, ReplicationTarget::S3 { .. }));
    }

    #[test]
    fn s3_empty_bucket_rejected() {
        let error = ReplicaConfig::try_from(s3_json("   ", "AKIA", None)).unwrap_err();
        assert!(
            matches!(error, super::ReplicaConfigError::EmptyS3Bucket),
            "expected EmptyS3Bucket, got {error:?}"
        );
    }

    #[test]
    fn s3_empty_access_key_rejected() {
        let error = ReplicaConfig::try_from(s3_json("bucket", "", None)).unwrap_err();
        assert!(
            matches!(error, super::ReplicaConfigError::EmptyS3AccessKeyId),
            "expected EmptyS3AccessKeyId, got {error:?}"
        );
    }

    #[test]
    fn s3_whitespace_secret_rejected() {
        let mut json = s3_json("bucket", "AKIA", None);
        let ReplicationTarget::S3 {
            secret_access_key, ..
        } = &mut json.target
        else {
            panic!("s3_json builds an S3 target");
        };
        *secret_access_key = "   ".parse().expect("whitespace passes Secret validation");
        let error = ReplicaConfig::try_from(json).unwrap_err();
        assert!(
            matches!(error, super::ReplicaConfigError::EmptyS3SecretAccessKey),
            "expected EmptyS3SecretAccessKey, got {error:?}"
        );
    }

    #[test]
    fn s3_malformed_endpoint_rejected() {
        let error =
            ReplicaConfig::try_from(s3_json("bucket", "AKIA", Some("not a url"))).unwrap_err();
        assert!(
            matches!(error, super::ReplicaConfigError::InvalidS3Endpoint(_)),
            "expected InvalidS3Endpoint, got {error:?}"
        );
    }

    #[test]
    fn file_target_builds_local_storage() {
        let config = ReplicaConfig::try_from(file_json(|_| {})).unwrap();
        let storage = config.build_storage();
        assert!(matches!(storage, crate::storage::ReplicaStorage::Local(_)));
    }
}
