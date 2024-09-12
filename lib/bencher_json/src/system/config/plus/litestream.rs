use std::path::PathBuf;

use bencher_valid::{Sanitize, Secret};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
// https://litestream.io/reference/config/#replica-settings
pub struct JsonLitestream {
    /// The database busy timeout in milliseconds
    pub busy_timeout: Option<u32>,
    /// Disaster recovery replicas
    pub replicas: Vec<JsonReplica>,
}

impl Sanitize for JsonLitestream {
    fn sanitize(&mut self) {
        for replica in &mut self.replicas {
            replica.sanitize();
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(tag = "scheme", rename_all = "snake_case")]
pub enum JsonReplica {
    // https://litestream.io/reference/config/#file-replica
    File {
        path: PathBuf,
        // Shared keys
        #[serde(skip_serializing_if = "Option::is_none")]
        retention: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        retention_check_interval: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        snapshot_interval: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        validation_interval: Option<String>,
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
        // Shared keys
        #[serde(skip_serializing_if = "Option::is_none")]
        retention: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        retention_check_interval: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        snapshot_interval: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        validation_interval: Option<String>,
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
        // Shared keys
        #[serde(skip_serializing_if = "Option::is_none")]
        retention: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        retention_check_interval: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        snapshot_interval: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        validation_interval: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        sync_interval: Option<String>,
    },
    // https://litestream.io/guides/azure/
    Abs {
        account_name: String,
        bucket: String,
        path: String,
        account_key: Secret,
        // Shared keys
        #[serde(skip_serializing_if = "Option::is_none")]
        retention: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        retention_check_interval: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        snapshot_interval: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        validation_interval: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        sync_interval: Option<String>,
    },
    // https://litestream.io/guides/gcs/
    Gcs {
        bucket: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        path: Option<String>,
        // Shared keys
        #[serde(skip_serializing_if = "Option::is_none")]
        retention: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        retention_check_interval: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        snapshot_interval: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        validation_interval: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        sync_interval: Option<String>,
    },
}

impl Sanitize for JsonReplica {
    fn sanitize(&mut self) {
        match self {
            Self::File { .. } | Self::Gcs { .. } => {},
            Self::Sftp { password, .. } => password.sanitize(),
            Self::S3 {
                secret_access_key, ..
            } => secret_access_key.sanitize(),
            Self::Abs { account_key, .. } => account_key.sanitize(),
        }
    }
}

#[cfg(feature = "db")]
mod db {
    use std::path::PathBuf;

    use bencher_valid::Secret;
    use serde::Serialize;

    use crate::system::config::LogLevel;

    use super::{JsonLitestream, JsonReplica};

    impl JsonLitestream {
        pub fn into_yaml(
            self,
            path: PathBuf,
            log_level: LogLevel,
        ) -> Result<String, serde_yaml::Error> {
            let replicas = self
                .replicas
                .into_iter()
                .map(LitestreamReplica::from)
                .collect();
            let dbs = vec![LitestreamDb { path, replicas }];
            let logging = Some(LitestreamLogging {
                level: Some(log_level.into()),
            });
            let litestream = Litestream { dbs, logging };
            serde_yaml::to_string(&litestream)
        }
    }

    #[derive(Debug, Clone, Serialize)]
    #[serde(rename_all = "kebab-case")]
    pub struct Litestream {
        pub dbs: Vec<LitestreamDb>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub logging: Option<LitestreamLogging>,
    }

    #[derive(Debug, Clone, Serialize)]
    #[serde(rename_all = "kebab-case")]
    pub struct LitestreamDb {
        pub path: PathBuf,
        pub replicas: Vec<LitestreamReplica>,
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
            // Shared keys
            #[serde(skip_serializing_if = "Option::is_none")]
            retention: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            retention_check_interval: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            snapshot_interval: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            validation_interval: Option<String>,
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
            // Shared keys
            #[serde(skip_serializing_if = "Option::is_none")]
            retention: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            retention_check_interval: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            snapshot_interval: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            validation_interval: Option<String>,
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
            // Shared keys
            #[serde(skip_serializing_if = "Option::is_none")]
            retention: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            retention_check_interval: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            snapshot_interval: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            validation_interval: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            sync_interval: Option<String>,
        },
        Abs {
            account_name: String,
            bucket: String,
            path: String,
            account_key: Secret,
            // Shared keys
            #[serde(skip_serializing_if = "Option::is_none")]
            retention: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            retention_check_interval: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            snapshot_interval: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            validation_interval: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            sync_interval: Option<String>,
        },
        Gcs {
            bucket: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            path: Option<String>,
            // Shared keys
            #[serde(skip_serializing_if = "Option::is_none")]
            retention: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            retention_check_interval: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            snapshot_interval: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            validation_interval: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            sync_interval: Option<String>,
        },
    }

    impl From<JsonReplica> for LitestreamReplica {
        #[allow(clippy::too_many_lines)]
        fn from(replica: JsonReplica) -> Self {
            match replica {
                JsonReplica::File {
                    path,
                    retention,
                    retention_check_interval,
                    snapshot_interval,
                    validation_interval,
                    sync_interval,
                } => Self::File {
                    path,
                    retention,
                    retention_check_interval,
                    snapshot_interval,
                    validation_interval,
                    sync_interval,
                },
                JsonReplica::Sftp {
                    host,
                    port,
                    user,
                    password,
                    path,
                    key_path,
                    retention,
                    retention_check_interval,
                    snapshot_interval,
                    validation_interval,
                    sync_interval,
                } => Self::Sftp {
                    host: format!("{host}:{port}"),
                    user,
                    password,
                    path,
                    key_path,
                    retention,
                    retention_check_interval,
                    snapshot_interval,
                    validation_interval,
                    sync_interval,
                },
                JsonReplica::S3 {
                    bucket,
                    path,
                    endpoint,
                    region,
                    access_key_id,
                    secret_access_key,
                    retention,
                    retention_check_interval,
                    snapshot_interval,
                    validation_interval,
                    sync_interval,
                } => Self::S3 {
                    bucket,
                    path,
                    endpoint,
                    region,
                    access_key_id,
                    secret_access_key,
                    retention,
                    retention_check_interval,
                    snapshot_interval,
                    validation_interval,
                    sync_interval,
                },
                JsonReplica::Abs {
                    account_name,
                    bucket,
                    path,
                    account_key,
                    retention,
                    retention_check_interval,
                    snapshot_interval,
                    validation_interval,
                    sync_interval,
                } => Self::Abs {
                    account_name,
                    bucket,
                    path,
                    account_key,
                    retention,
                    retention_check_interval,
                    snapshot_interval,
                    validation_interval,
                    sync_interval,
                },
                JsonReplica::Gcs {
                    bucket,
                    path,
                    retention,
                    retention_check_interval,
                    snapshot_interval,
                    validation_interval,
                    sync_interval,
                } => Self::Gcs {
                    bucket,
                    path,
                    retention,
                    retention_check_interval,
                    snapshot_interval,
                    validation_interval,
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
    fn test_into_yaml() {
        let json_litestream = JsonLitestream {
            busy_timeout: None,
            replicas: vec![JsonReplica::S3 {
                bucket: "bucket".to_owned(),
                path: Some("/path/to/backup".to_owned()),
                endpoint: None,
                region: None,
                access_key_id: "access_key_id".to_owned(),
                secret_access_key: "secret_access_key".parse().unwrap(),
                retention: None,
                retention_check_interval: None,
                snapshot_interval: None,
                validation_interval: None,
                sync_interval: None,
            }],
        };
        let path = PathBuf::from("/path/to/db");
        let log_level = LogLevel::Info;
        let yaml = json_litestream.into_yaml(path, log_level).unwrap();
        pretty_assertions::assert_eq!(
            yaml,
            "dbs:
- path: /path/to/db
  replicas:
  - type: s3
    bucket: bucket
    path: /path/to/backup
    access-key-id: access_key_id
    secret-access-key: secret_access_key
logging:
  level: info
"
        );
    }
}
