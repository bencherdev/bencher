//! Litestream configuration and runtime for Bencher Plus.
//!
//! The wire types ([`JsonLitestream`] and friends) render to Litestream's YAML
//! config (`yaml` feature), and [`run_litestream`] supervises `litestream restore`
//! then `litestream replicate` around a `SQLite` database replicated to
//! S3-compatible storage (`runtime` feature). The types live in this crate, rather
//! than `bencher_json`, so they can be shared by path without pulling in `bencher_json`.
//!
//! <https://litestream.io/reference/config/>

use std::path::PathBuf;

use bencher_valid::{Sanitize, Secret};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[cfg(feature = "runtime")]
mod runtime;
#[cfg(feature = "yaml")]
mod yaml;

#[cfg(feature = "runtime")]
pub use runtime::{LitestreamError, run_litestream};

/// `SQLite` PRAGMA that disables automatic checkpoints so that Litestream manages them.
///
/// Apply this on the writer connection whenever Litestream is enabled.
///
/// <https://litestream.io/tips/#disable-autocheckpoints-for-high-write-load-servers>
/// <https://sqlite.org/wal.html#automatic_checkpoint>
pub const DISABLE_AUTOCHECKPOINT_PRAGMA: &str = "PRAGMA wal_autocheckpoint = 0";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
// https://litestream.io/reference/config/
pub struct JsonLitestream {
    /// Disaster recovery replica
    pub replica: JsonReplica,
    /// Snapshot configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<JsonSnapshot>,
    /// Validation configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation: Option<JsonValidation>,
    /// Checkpoint configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkpoint: Option<JsonCheckpoint>,
}

impl Sanitize for JsonLitestream {
    fn sanitize(&mut self) {
        self.replica.sanitize();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonSnapshot {
    /// How often new snapshots are created
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval: Option<String>,
    /// How long snapshot & WAL files are kept
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retention: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonValidation {
    /// How often to restore and validate replica data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonCheckpoint {
    /// How often to perform a non-blocking PASSIVE checkpoint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval: Option<String>,
    /// Minimum WAL pages before a PASSIVE checkpoint triggers
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_page_count: Option<u64>,
    /// Page threshold for blocking TRUNCATE checkpoint (0 to disable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncate_page_n: Option<u64>,
}

impl JsonCheckpoint {
    /// Default value for `truncate_page_n`: disables blocking TRUNCATE checkpoints.
    /// When set to 0, Litestream will never perform a blocking TRUNCATE checkpoint,
    /// only non-blocking PASSIVE checkpoints (controlled by `interval` / `min_page_count`).
    pub const TRUNCATE_DISABLED: u64 = 0;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(tag = "scheme", rename_all = "snake_case")]
pub enum JsonReplica {
    // https://litestream.io/reference/config/#file-replica
    File {
        path: PathBuf,
        #[serde(skip_serializing_if = "Option::is_none")]
        sync_interval: Option<String>,
    },
    // https://litestream.io/guides/sftp/
    Sftp {
        host: String,
        port: u16,
        user: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        password: Option<Secret>,
        #[serde(skip_serializing_if = "Option::is_none")]
        path: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        key_path: Option<PathBuf>,
        #[serde(skip_serializing_if = "Option::is_none")]
        sync_interval: Option<String>,
    },
    // https://litestream.io/guides/s3/
    S3 {
        bucket: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        path: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        endpoint: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        region: Option<String>,
        access_key_id: String,
        secret_access_key: Secret,
        #[serde(skip_serializing_if = "Option::is_none")]
        sync_interval: Option<String>,
    },
}

impl Sanitize for JsonReplica {
    fn sanitize(&mut self) {
        match self {
            Self::File { .. } => {},
            Self::Sftp { password, .. } => password.sanitize(),
            Self::S3 {
                secret_access_key, ..
            } => secret_access_key.sanitize(),
        }
    }
}

/// Litestream log level.
///
/// Mirrors the values Litestream accepts for `logging.level`. Construct it directly,
/// or, in `bencher`, from a `bencher_json` `LogLevel` via its `From` impl.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum LitestreamLevel {
    Debug,
    Info,
    Warn,
    Error,
}

#[cfg(test)]
mod tests {
    use bencher_valid::Sanitize as _;

    use super::JsonReplica;

    #[test]
    fn sanitize_file() {
        let mut replica = JsonReplica::File {
            path: std::path::PathBuf::from("/path/to/replica"),
            sync_interval: None,
        };
        replica.sanitize();
        let JsonReplica::File { path, .. } = replica else {
            panic!("expected File")
        };
        assert_eq!(path, std::path::PathBuf::from("/path/to/replica"));
    }

    #[test]
    fn sanitize_sftp() {
        let mut replica = JsonReplica::Sftp {
            host: "example.com".to_owned(),
            port: 22,
            user: "user".to_owned(),
            password: Some("secret_pass".parse().unwrap()),
            path: None,
            key_path: None,
            sync_interval: None,
        };
        replica.sanitize();
        let JsonReplica::Sftp { password, .. } = replica else {
            panic!("expected Sftp")
        };
        assert_eq!(password.unwrap().as_ref(), "************");
    }

    #[test]
    fn sanitize_s3() {
        let mut replica = JsonReplica::S3 {
            bucket: "mybucket".to_owned(),
            path: None,
            endpoint: None,
            region: None,
            access_key_id: "access_key_id".to_owned(),
            secret_access_key: "my_secret".parse().unwrap(),
            sync_interval: None,
        };
        replica.sanitize();
        let JsonReplica::S3 {
            secret_access_key, ..
        } = replica
        else {
            panic!("expected S3")
        };
        assert_eq!(secret_access_key.as_ref(), "************");
    }
}
