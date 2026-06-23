use std::path::PathBuf;

use bencher_valid::Secret;
use camino::Utf8PathBuf;
use serde::Serialize;

use crate::{
    JsonCheckpoint, JsonLitestream, JsonReplica, JsonSnapshot, JsonValidation, LitestreamLevel,
};

impl JsonLitestream {
    pub fn into_yaml(
        self,
        path: Utf8PathBuf,
        log_level: LitestreamLevel,
    ) -> Result<String, serde_yaml::Error> {
        let Self {
            replica,
            snapshot,
            validation,
            checkpoint,
        } = self;
        let replica = LitestreamReplica::from(replica);
        let snapshot = snapshot.map(LitestreamSnapshot::from);
        let validation = validation.map(LitestreamValidation::from);
        let (min_checkpoint_page_count, checkpoint_interval, truncate_page_n) =
            checkpoint.map_or((None, None, JsonCheckpoint::TRUNCATE_DISABLED), |c| {
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
            level: Some(log_level),
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
struct Litestream {
    dbs: Vec<LitestreamDb>,
    #[serde(skip_serializing_if = "Option::is_none")]
    snapshot: Option<LitestreamSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    validation: Option<LitestreamValidation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    logging: Option<LitestreamLogging>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
struct LitestreamDb {
    path: Utf8PathBuf,
    replica: LitestreamReplica,
    #[serde(skip_serializing_if = "Option::is_none")]
    min_checkpoint_page_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    checkpoint_interval: Option<String>,
    truncate_page_n: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(
    tag = "type",
    rename_all = "kebab-case",
    rename_all_fields = "kebab-case"
)]
enum LitestreamReplica {
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
struct LitestreamSnapshot {
    #[serde(skip_serializing_if = "Option::is_none")]
    interval: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    retention: Option<String>,
}

impl From<JsonSnapshot> for LitestreamSnapshot {
    fn from(snapshot: JsonSnapshot) -> Self {
        let JsonSnapshot {
            interval,
            retention,
        } = snapshot;
        Self {
            interval,
            retention,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
struct LitestreamValidation {
    #[serde(skip_serializing_if = "Option::is_none")]
    interval: Option<String>,
}

impl From<JsonValidation> for LitestreamValidation {
    fn from(validation: JsonValidation) -> Self {
        let JsonValidation { interval } = validation;
        Self { interval }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
struct LitestreamLogging {
    #[serde(skip_serializing_if = "Option::is_none")]
    level: Option<LitestreamLevel>,
}

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;

    use crate::{
        JsonCheckpoint, JsonLitestream, JsonReplica, JsonSnapshot, JsonValidation, LitestreamLevel,
    };

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
        let path = Utf8PathBuf::from("/path/to/db");
        let yaml = json_litestream
            .into_yaml(path, LitestreamLevel::Info)
            .unwrap();
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
        let path = Utf8PathBuf::from("/path/to/db");
        let yaml = json_litestream
            .into_yaml(path, LitestreamLevel::Info)
            .unwrap();
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
        let path = Utf8PathBuf::from("/path/to/db");
        let yaml = json_litestream
            .into_yaml(path, LitestreamLevel::Info)
            .unwrap();
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
                path: std::path::PathBuf::from("/path/to/replica"),
                sync_interval: Some("5s".to_owned()),
            },
            snapshot: None,
            validation: None,
            checkpoint: None,
        };
        let path = Utf8PathBuf::from("/path/to/db");
        let yaml = json_litestream
            .into_yaml(path, LitestreamLevel::Info)
            .unwrap();
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
        let path = Utf8PathBuf::from("/path/to/db");
        let yaml = json_litestream
            .into_yaml(path, LitestreamLevel::Info)
            .unwrap();
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
}
