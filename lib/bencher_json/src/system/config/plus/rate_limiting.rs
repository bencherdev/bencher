#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonRateLimiting {
    pub window: Option<u32>,
    pub user_limit: Option<u32>,
    pub unclaimed_limit: Option<u32>,
    pub claimed_limit: Option<u32>,
    pub public: Option<JsonPublicRateLimiter>,
    pub user: Option<JsonUserRateLimiter>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPublicRateLimiter {
    pub requests: Option<JsonRateLimits>,
    pub runs: Option<JsonRateLimits>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUserRateLimiter {
    pub requests: Option<JsonRateLimits>,
    pub attempts: Option<JsonRateLimits>,
    pub invites: Option<JsonRateLimits>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonRateLimits {
    pub minute: Option<usize>,
    pub hour: Option<usize>,
    pub day: Option<usize>,
}
