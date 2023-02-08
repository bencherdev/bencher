use std::{net::SocketAddr, path::PathBuf};

use bencher_valid::{Sanitize, Secret};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUpdateConfig {
    pub config: JsonConfig,
    pub delay: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConfig {
    pub endpoint: Url,
    pub secret_key: Secret,
    pub server: JsonServer,
    pub logging: JsonLogging,
    pub database: JsonDatabase,
    pub smtp: Option<JsonSmtp>,
    #[cfg(feature = "plus")]
    pub bencher: Option<JsonBencher>,
}

impl Sanitize for JsonConfig {
    fn sanitize(&mut self) {
        self.secret_key.sanitize();
        self.database.sanitize();
        if let Some(smtp) = &mut self.smtp {
            smtp.sanitize();
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonServer {
    pub bind_address: SocketAddr,
    pub request_body_max_bytes: usize,
    pub tls: Option<JsonTls>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum JsonTls {
    AsFile {
        cert_file: PathBuf,
        key_file: PathBuf,
    },
    AsBytes {
        certs: Vec<u8>,
        key: Vec<u8>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonDatabase {
    pub file: PathBuf,
    pub data_store: Option<DataStore>,
}

impl Sanitize for JsonDatabase {
    fn sanitize(&mut self) {
        if let Some(data_store) = &mut self.data_store {
            data_store.sanitize();
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(tag = "service", rename_all = "snake_case")]
pub enum DataStore {
    AwsS3 {
        access_key_id: String,
        secret_access_key: Secret,
        // arn:aws:s3:<region>:<account-id>:accesspoint/<resource>[/backup-dir-path]
        // https://docs.aws.amazon.com/AmazonS3/latest/userguide/using-access-points.html
        access_point: String,
    },
}

impl Sanitize for DataStore {
    fn sanitize(&mut self) {
        match self {
            Self::AwsS3 {
                secret_access_key, ..
            } => secret_access_key.sanitize(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonSmtp {
    pub hostname: String,
    pub port: Option<u16>,
    pub starttls: Option<bool>,
    pub username: String,
    pub secret: Secret,
    pub from_name: String,
    pub from_email: String,
}

impl Sanitize for JsonSmtp {
    fn sanitize(&mut self) {
        self.secret.sanitize();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonLogging {
    pub name: String,
    pub log: ServerLog,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ServerLog {
    StderrTerminal {
        level: LogLevel,
    },
    File {
        level: LogLevel,
        path: String,
        if_exists: IfExists,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum IfExists {
    Fail,
    Truncate,
    Append,
}

#[cfg(feature = "plus")]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonBencher {
    pub private_pem: Secret,
}
