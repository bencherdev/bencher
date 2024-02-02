use std::path::PathBuf;

use bencher_valid::{Sanitize, Secret, Url};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
// https://litestream.io/reference/config/#replica-settings
pub struct JsonLitestream {
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
#[serde(tag = "service", rename_all = "snake_case")]
pub enum JsonReplica {
    // https://litestream.io/guides/s3/
    AwsS3 {
        access_key_id: String,
        secret_access_key: Secret,
        url: Url,
    },
    // https://litestream.io/guides/azure/
    AzureBlobStorage {
        account_key: Secret,
        url: Url,
    },
    // https://litestream.io/reference/config/#file-replica
    File {
        path: PathBuf,
    },
    // https://litestream.io/guides/sftp/
    Sftp {
        key_path: Option<PathBuf>,
        url: Url,
    },
}

impl Sanitize for JsonReplica {
    fn sanitize(&mut self) {
        match self {
            Self::AwsS3 {
                secret_access_key, ..
            } => secret_access_key.sanitize(),
            Self::AzureBlobStorage { account_key, .. } => account_key.sanitize(),
            Self::File { .. } | Self::Sftp { .. } => {},
        }
    }
}

#[cfg(feature = "db")]
mod db {
    use std::path::PathBuf;

    use bencher_valid::{Secret, Url};
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
    #[serde(untagged, rename_all_fields = "kebab-case")]
    pub enum LitestreamReplica {
        S3 {
            access_key_id: String,
            secret_access_key: Secret,
            url: Url,
        },
        Abs {
            account_key: Secret,
            url: Url,
        },
        File {
            path: PathBuf,
        },
        Sftp {
            #[serde(skip_serializing_if = "Option::is_none")]
            key_path: Option<PathBuf>,
            url: Url,
        },
    }

    impl From<JsonReplica> for LitestreamReplica {
        fn from(replica: JsonReplica) -> Self {
            match replica {
                JsonReplica::AwsS3 {
                    access_key_id,
                    secret_access_key,
                    url,
                } => Self::S3 {
                    access_key_id,
                    secret_access_key,
                    url,
                },
                JsonReplica::AzureBlobStorage { account_key, url } => {
                    Self::Abs { account_key, url }
                },
                JsonReplica::File { path } => Self::File { path },
                JsonReplica::Sftp { key_path, url } => Self::Sftp { key_path, url },
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
    #[allow(clippy::unwrap_used)]
    fn test_into_yaml() {
        let json_litestream = JsonLitestream {
            replicas: vec![JsonReplica::AwsS3 {
                access_key_id: "access_key_id".to_owned(),
                secret_access_key: "secret_access_key".parse().unwrap(),
                url: "https://example.com".parse().unwrap(),
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
  - type: aws-s3
    access-key-id: access_key_id
    secret-access-key: secret_access_key
    url: https://example.com
logging:
  level: info
"
        );
    }
}
