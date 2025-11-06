use std::time::Duration;

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
    /// The OTEL export interval
    pub interval: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum OtelProtocol {
    #[serde(rename = "http/protobuf")]
    HttpProtobuf,
}
