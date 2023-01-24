use std::{net::SocketAddr, path::PathBuf};

use bencher_valid::NonEmpty;
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
    pub secret_key: Option<String>,
    pub server: JsonServer,
    pub database: JsonDatabase,
    pub smtp: Option<JsonSmtp>,
    pub logging: JsonLogging,
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
pub struct JsonTls {
    pub cert_file: PathBuf,
    pub key_file: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonDatabase {
    pub file: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(untagged)]
pub enum DataStore {
    AwsS3 {
        access_key_id: NonEmpty,
        secret_access_key: NonEmpty,
        // arn:aws:s3:<region>:<account-id>:accesspoint/<resource>
        // https://docs.aws.amazon.com/AmazonS3/latest/userguide/using-access-points.html
        access_point: NonEmpty,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonSmtp {
    pub hostname: String,
    pub username: String,
    pub secret: String,
    pub from_name: String,
    pub from_email: String,
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
