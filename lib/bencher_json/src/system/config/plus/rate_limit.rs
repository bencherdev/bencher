#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonRateLimit {
    pub window: Option<u32>,
    pub unclaimed: Option<u32>,
    pub claimed: Option<u32>,
}
