use std::path::PathBuf;

use bencher_valid::{Sanitize, Secret};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// In-process disaster recovery replication (`bencher_replica`).
///
/// Streams `SQLite` WAL frames to a replica target, takes periodic snapshots,
/// and restores the latest state at API server startup. Replaces Litestream.
/// When both `litestream` and `replica` are configured, the replica runs in
/// shadow mode: Litestream keeps checkpoint ownership and remains the restore
/// source.
///
/// Exactly one server may replicate to a given target. There is no fencing
/// between concurrent writers, so pointing two servers at the same target
/// interleaves their lineages and corrupts the replica.
///
/// `deny_unknown_fields` is deliberate: this is disaster-recovery config, so a
/// misspelled key (e.g. `sync_interval` for `sync_interval_secs`) must fail
/// loudly at load time instead of silently applying a default.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct JsonReplication {
    /// Replica target
    pub target: ReplicationTarget,
    /// How often to ship new WAL frames (seconds; default 1)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sync_interval_secs: Option<u32>,
    /// Minimum interval between checkpoints (seconds; default 60)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkpoint_interval_secs: Option<u32>,
    /// Minimum WAL pages before a checkpoint triggers (default 1000)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_checkpoint_pages: Option<u32>,
    /// How often to start a new generation with a fresh snapshot
    /// (seconds; default 86400)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot_interval_secs: Option<u32>,
    /// Snapshot copy throttle (MiB per second; default 32)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot_throttle_mib: Option<u32>,
    /// Number of generations to retain (default 3)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retention_generations: Option<u32>,
    /// How often to run restore-and-compare verification
    /// (seconds; default 86400, 0 disables). Each run restores a full copy of
    /// the database next to the live one, so expect a transient doubling of
    /// data-volume usage while it runs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification_interval_secs: Option<u32>,
    /// Deadline for the final WAL ship at shutdown (seconds; default 4,
    /// which fits inside Fly's 6 second kill timeout)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shutdown_sync_timeout_secs: Option<u32>,
}

impl Sanitize for JsonReplication {
    fn sanitize(&mut self) {
        self.target.sanitize();
    }
}

/// Where the replica lives: a local directory XOR S3-compatible storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(tag = "scheme", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReplicationTarget {
    File {
        path: PathBuf,
    },
    /// S3-compatible object storage. Configure a lifecycle rule on the bucket
    /// to abort incomplete multipart uploads: a crashed snapshot upload leaves
    /// orphaned parts that otherwise accrue storage cost indefinitely.
    S3 {
        bucket: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        path: Option<String>,
        /// Endpoint override for S3-compatible services (R2, `MinIO`)
        #[serde(skip_serializing_if = "Option::is_none")]
        endpoint: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        region: Option<String>,
        access_key_id: String,
        secret_access_key: Secret,
    },
}

impl Sanitize for ReplicationTarget {
    fn sanitize(&mut self) {
        match self {
            Self::File { .. } => {},
            Self::S3 {
                secret_access_key, ..
            } => secret_access_key.sanitize(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replication_json_round_trip() {
        let json = r#"{
            "target": {
                "scheme": "s3",
                "bucket": "my-bucket",
                "path": "replica/prod",
                "endpoint": "https://minio.example.com",
                "region": "auto",
                "access_key_id": "access_key_id",
                "secret_access_key": "secret_access_key"
            },
            "snapshot_interval_secs": 86400
        }"#;
        let replication: JsonReplication = serde_json::from_str(json).unwrap();
        let ReplicationTarget::S3 {
            bucket,
            path,
            endpoint,
            region,
            ..
        } = &replication.target
        else {
            panic!("expected S3 target");
        };
        pretty_assertions::assert_eq!(bucket, "my-bucket");
        pretty_assertions::assert_eq!(path.as_deref(), Some("replica/prod"));
        pretty_assertions::assert_eq!(endpoint.as_deref(), Some("https://minio.example.com"));
        pretty_assertions::assert_eq!(region.as_deref(), Some("auto"));
        pretty_assertions::assert_eq!(replication.snapshot_interval_secs, Some(86_400));
        pretty_assertions::assert_eq!(replication.sync_interval_secs, None);
    }

    // A misspelled top-level field (e.g. `sync_interval` for
    // `sync_interval_secs`) must fail loudly, not silently apply a default.
    #[test]
    fn replication_rejects_unknown_top_level_field() {
        let json = r#"{
            "target": { "scheme": "file", "path": "/var/lib/bencher/replica" },
            "sync_interval": 5
        }"#;
        let error = serde_json::from_str::<JsonReplication>(json)
            .expect_err("misspelled field must fail deserialization");
        assert!(
            error.to_string().contains("sync_interval"),
            "error should name the unknown field, got: {error}"
        );
    }

    // A misspelled field inside the target section must also fail loudly.
    #[test]
    fn replication_rejects_unknown_target_field() {
        let json = r#"{
            "target": {
                "scheme": "s3",
                "buckett": "my-bucket",
                "access_key_id": "access_key_id",
                "secret_access_key": "secret_access_key"
            }
        }"#;
        let error = serde_json::from_str::<JsonReplication>(json)
            .expect_err("misspelled target field must fail deserialization");
        assert!(
            error.to_string().contains("buckett"),
            "error should name the unknown field, got: {error}"
        );
    }

    // The correct config still parses cleanly.
    #[test]
    fn replication_accepts_known_fields() {
        let json = r#"{
            "target": { "scheme": "file", "path": "/var/lib/bencher/replica" },
            "sync_interval_secs": 5,
            "verification_interval_secs": 0
        }"#;
        let replication: JsonReplication =
            serde_json::from_str(json).expect("valid config must parse");
        pretty_assertions::assert_eq!(replication.sync_interval_secs, Some(5));
        pretty_assertions::assert_eq!(replication.verification_interval_secs, Some(0));
    }

    #[test]
    fn replication_file_target_round_trip() {
        let json = r#"{ "target": { "scheme": "file", "path": "/var/lib/bencher/replica" } }"#;
        let replication: JsonReplication = serde_json::from_str(json).unwrap();
        let ReplicationTarget::File { path } = &replication.target else {
            panic!("expected File target");
        };
        pretty_assertions::assert_eq!(path, &PathBuf::from("/var/lib/bencher/replica"));
    }

    #[test]
    fn sanitize_s3_secret() {
        let mut replication = JsonReplication {
            target: ReplicationTarget::S3 {
                bucket: "bucket".to_owned(),
                path: None,
                endpoint: None,
                region: None,
                access_key_id: "access_key_id".to_owned(),
                secret_access_key: "my_secret".parse().unwrap(),
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
        replication.sanitize();
        let ReplicationTarget::S3 {
            secret_access_key, ..
        } = &replication.target
        else {
            panic!("expected S3 target");
        };
        pretty_assertions::assert_eq!(secret_access_key.as_ref(), "************");
    }
}
