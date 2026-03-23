use std::path::PathBuf;

use bencher_valid::{Sanitize, Secret};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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

impl Sanitize for JsonLitestream {
    fn sanitize(&mut self) {
        self.replica.sanitize();
    }
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

#[cfg(feature = "db")]
mod db {
    use std::path::PathBuf;

    use bencher_valid::Secret;
    use serde::Serialize;

    use crate::system::config::LogLevel;

    use super::{JsonCheckpoint, JsonLitestream, JsonReplica, JsonSnapshot, JsonValidation};

    impl JsonLitestream {
        pub fn into_yaml(
            self,
            path: PathBuf,
            log_level: LogLevel,
        ) -> Result<String, serde_yaml::Error> {
            let Self {
                replica,
                snapshot,
                validation,
                checkpoint,
            } = self;
            let replica = LitestreamReplica::from(replica);
            let snapshot = snapshot.map(|s| {
                let JsonSnapshot {
                    interval,
                    retention,
                } = s;
                LitestreamSnapshot {
                    interval,
                    retention,
                }
            });
            let validation = validation.map(|v| {
                let JsonValidation { interval } = v;
                LitestreamValidation { interval }
            });
            let (min_checkpoint_page_count, checkpoint_interval, truncate_page_n) = checkpoint
                .map_or((None, None, JsonCheckpoint::TRUNCATE_DISABLED), |c| {
                    let JsonCheckpoint {
                        interval,
                        min_page_count,
                        truncate_page_n,
                    } = c;
                    (
                        min_page_count,
                        interval,
                        truncate_page_n.unwrap_or(JsonCheckpoint::TRUNCATE_DISABLED),
                    )
                });
            let dbs = vec![LitestreamDb {
                path,
                replica,
                min_checkpoint_page_count,
                checkpoint_interval,
                truncate_page_n,
            }];
            let logging = Some(LitestreamLogging {
                level: Some(log_level.into()),
            });
            let litestream = Litestream {
                dbs,
                snapshot,
                validation,
                logging,
            };
            serde_yaml::to_string(&litestream)
        }
    }

    #[derive(Debug, Clone, Serialize)]
    #[serde(rename_all = "kebab-case")]
    pub struct Litestream {
        pub dbs: Vec<LitestreamDb>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub snapshot: Option<LitestreamSnapshot>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub validation: Option<LitestreamValidation>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub logging: Option<LitestreamLogging>,
    }

    #[derive(Debug, Clone, Serialize)]
    #[serde(rename_all = "kebab-case")]
    pub struct LitestreamSnapshot {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub interval: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub retention: Option<String>,
    }

    #[derive(Debug, Clone, Serialize)]
    #[serde(rename_all = "kebab-case")]
    pub struct LitestreamValidation {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub interval: Option<String>,
    }

    #[derive(Debug, Clone, Serialize)]
    #[serde(rename_all = "kebab-case")]
    pub struct LitestreamDb {
        pub path: PathBuf,
        pub replica: LitestreamReplica,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub min_checkpoint_page_count: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub checkpoint_interval: Option<String>,
        pub truncate_page_n: u64,
    }

    #[derive(Debug, Clone, Serialize)]
    #[serde(
        tag = "type",
        rename_all = "kebab-case",
        rename_all_fields = "kebab-case"
    )]
    pub enum LitestreamReplica {
        File {
            path: PathBuf,
            #[serde(skip_serializing_if = "Option::is_none")]
            sync_interval: Option<String>,
        },
        Sftp {
            host: String,
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

    impl From<JsonReplica> for LitestreamReplica {
        fn from(replica: JsonReplica) -> Self {
            match replica {
                JsonReplica::File {
                    path,
                    sync_interval,
                } => Self::File {
                    path,
                    sync_interval,
                },
                JsonReplica::Sftp {
                    host,
                    port,
                    user,
                    password,
                    path,
                    key_path,
                    sync_interval,
                } => Self::Sftp {
                    host: format!("{host}:{port}"),
                    user,
                    password,
                    path,
                    key_path,
                    sync_interval,
                },
                JsonReplica::S3 {
                    bucket,
                    path,
                    endpoint,
                    region,
                    access_key_id,
                    secret_access_key,
                    sync_interval,
                } => Self::S3 {
                    bucket,
                    path,
                    endpoint,
                    region,
                    access_key_id,
                    secret_access_key,
                    sync_interval,
                },
            }
        }
    }

    #[derive(Debug, Clone, Serialize)]
    #[serde(rename_all = "kebab-case")]
    pub struct LitestreamLogging {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub level: Option<LitestreamLevel>,
    }

    #[derive(Debug, Clone, Serialize)]
    #[serde(rename_all = "kebab-case")]
    pub enum LitestreamLevel {
        Debug,
        Info,
        Warn,
        Error,
    }

    impl From<LogLevel> for LitestreamLevel {
        fn from(level: LogLevel) -> Self {
            match level {
                LogLevel::Trace | LogLevel::Debug => Self::Debug,
                LogLevel::Info => Self::Info,
                LogLevel::Warn => Self::Warn,
                LogLevel::Error | LogLevel::Critical => Self::Error,
            }
        }
    }

    #[test]
    fn into_yaml() {
        let json_litestream = JsonLitestream {
            replica: JsonReplica::S3 {
                bucket: "bucket".to_owned(),
                path: Some("/path/to/backup".to_owned()),
                endpoint: None,
                region: None,
                access_key_id: "access_key_id".to_owned(),
                secret_access_key: "secret_access_key".parse().unwrap(),
                sync_interval: None,
            },
            snapshot: None,
            validation: None,
            checkpoint: None,
        };
        let path = PathBuf::from("/path/to/db");
        let log_level = LogLevel::Info;
        let yaml = json_litestream.into_yaml(path, log_level).unwrap();
        pretty_assertions::assert_eq!(
            yaml,
            "dbs:
- path: /path/to/db
  replica:
    type: s3
    bucket: bucket
    path: /path/to/backup
    access-key-id: access_key_id
    secret-access-key: secret_access_key
  truncate-page-n: 0
logging:
  level: info
"
        );
    }

    #[test]
    fn into_yaml_with_snapshot_and_validation() {
        let json_litestream = JsonLitestream {
            replica: JsonReplica::S3 {
                bucket: "bucket".to_owned(),
                path: Some("/path/to/backup".to_owned()),
                endpoint: None,
                region: None,
                access_key_id: "access_key_id".to_owned(),
                secret_access_key: "secret_access_key".parse().unwrap(),
                sync_interval: None,
            },
            snapshot: Some(JsonSnapshot {
                interval: Some("1h".to_owned()),
                retention: Some("24h".to_owned()),
            }),
            validation: Some(JsonValidation {
                interval: Some("6h".to_owned()),
            }),
            checkpoint: None,
        };
        let path = PathBuf::from("/path/to/db");
        let log_level = LogLevel::Info;
        let yaml = json_litestream.into_yaml(path, log_level).unwrap();
        pretty_assertions::assert_eq!(
            yaml,
            "dbs:
- path: /path/to/db
  replica:
    type: s3
    bucket: bucket
    path: /path/to/backup
    access-key-id: access_key_id
    secret-access-key: secret_access_key
  truncate-page-n: 0
snapshot:
  interval: 1h
  retention: 24h
validation:
  interval: 6h
logging:
  level: info
"
        );
    }

    #[test]
    fn into_yaml_with_checkpoint_config() {
        let json_litestream = JsonLitestream {
            replica: JsonReplica::S3 {
                bucket: "bucket".to_owned(),
                path: Some("/path/to/backup".to_owned()),
                endpoint: None,
                region: None,
                access_key_id: "access_key_id".to_owned(),
                secret_access_key: "secret_access_key".parse().unwrap(),
                sync_interval: None,
            },
            snapshot: None,
            validation: None,
            checkpoint: Some(JsonCheckpoint {
                interval: Some("5m".to_owned()),
                min_page_count: Some(2000),
                truncate_page_n: Some(121_359),
            }),
        };
        let path = PathBuf::from("/path/to/db");
        let log_level = LogLevel::Info;
        let yaml = json_litestream.into_yaml(path, log_level).unwrap();
        pretty_assertions::assert_eq!(
            yaml,
            "dbs:
- path: /path/to/db
  replica:
    type: s3
    bucket: bucket
    path: /path/to/backup
    access-key-id: access_key_id
    secret-access-key: secret_access_key
  min-checkpoint-page-count: 2000
  checkpoint-interval: 5m
  truncate-page-n: 121359
logging:
  level: info
"
        );
    }

    #[test]
    fn into_yaml_file() {
        let json_litestream = JsonLitestream {
            replica: JsonReplica::File {
                path: PathBuf::from("/path/to/replica"),
                sync_interval: Some("5s".to_owned()),
            },
            snapshot: None,
            validation: None,
            checkpoint: None,
        };
        let path = PathBuf::from("/path/to/db");
        let log_level = LogLevel::Info;
        let yaml = json_litestream.into_yaml(path, log_level).unwrap();
        pretty_assertions::assert_eq!(
            yaml,
            "dbs:
- path: /path/to/db
  replica:
    type: file
    path: /path/to/replica
    sync-interval: 5s
  truncate-page-n: 0
logging:
  level: info
"
        );
    }

    #[test]
    fn into_yaml_sftp() {
        let json_litestream = JsonLitestream {
            replica: JsonReplica::Sftp {
                host: "example.com".to_owned(),
                port: 22,
                user: "user".to_owned(),
                password: Some("pass".parse().unwrap()),
                path: Some("/backup".to_owned()),
                key_path: None,
                sync_interval: None,
            },
            snapshot: None,
            validation: None,
            checkpoint: None,
        };
        let path = PathBuf::from("/path/to/db");
        let log_level = LogLevel::Info;
        let yaml = json_litestream.into_yaml(path, log_level).unwrap();
        pretty_assertions::assert_eq!(
            yaml,
            "dbs:
- path: /path/to/db
  replica:
    type: sftp
    host: example.com:22
    user: user
    password: pass
    path: /backup
  truncate-page-n: 0
logging:
  level: info
"
        );
    }

    #[test]
    fn sanitize_file() {
        use bencher_valid::Sanitize as _;
        let mut replica = JsonReplica::File {
            path: PathBuf::from("/path/to/replica"),
            sync_interval: None,
        };
        replica.sanitize();
        let JsonReplica::File { path, .. } = replica else {
            panic!("expected File")
        };
        assert_eq!(path, PathBuf::from("/path/to/replica"));
    }

    #[test]
    fn sanitize_sftp() {
        use bencher_valid::Sanitize as _;
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
        use bencher_valid::Sanitize as _;
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
