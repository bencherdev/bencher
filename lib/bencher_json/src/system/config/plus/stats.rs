#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonStats {
    /// Number of seconds from midnight
    pub offset: Option<u32>,
    /// Enable stats collection
    pub enabled: Option<bool>,
}
