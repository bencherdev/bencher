use bencher_valid::Url;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonOtel {
    /// The OTEL collector URL.
    pub endpoint: Url,
    /// The OTEL protocol.
    pub protocol: OtelProtocol,
    /// The OTEL export interval in seconds.
    /// Defaults to 15 seconds, if not specified.
    pub interval: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum OtelProtocol {
    /// GRPC protocol
    Grpc,
    /// HTTP protocol with binary protobuf
    #[serde(rename = "http/binary")]
    HttpBinary,
    /// HTTP protocol with JSON payload
    #[serde(rename = "http/json")]
    HttpJson,
}
