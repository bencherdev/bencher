#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonRestart {
    /// The delay in seconds before the server restarts.
    /// Defaults to 3 seconds, if not specified.
    pub delay: Option<u64>,
}
