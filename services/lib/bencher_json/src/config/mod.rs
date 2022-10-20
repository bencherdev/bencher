use std::{net::SocketAddr, path::PathBuf};

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConfig {
    pub endpoint: Url,
    pub secret_key: Option<String>,
    pub server: JsonServer,
    pub database: JsonDatabase,
    pub smtp: Option<JsonSmtp>,
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
pub struct JsonSmtp {
    pub hostname: String,
    pub username: String,
    pub secret: String,
    pub from_name: String,
    pub from_email: String,
}
