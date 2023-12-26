use std::{net::SocketAddr, path::PathBuf};

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonServer {
    pub bind_address: SocketAddr,
    pub request_body_max_bytes: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
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
