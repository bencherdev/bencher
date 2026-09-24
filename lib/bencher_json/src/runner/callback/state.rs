#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The delivery of a job's callback
#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonJobCallback {
    pub state: JobCallbackState,
    /// The HTTP status of the last response, if any attempt got one
    pub status: Option<u16>,
}

/// Job callback state
#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum JobCallbackState {
    Pending,
    Delivered,
    Failed,
    Skipped,
}
