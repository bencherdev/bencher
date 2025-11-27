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
    pub unclaimed_run_limit: Option<u32>,
    pub auth_window: Option<u32>,
    pub auth_limit: Option<u32>,
    pub requests: Option<JsonRequestsRateLimiter>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonRequestsRateLimiter {
    pub public: Option<JsonRateLimits>,
    pub user: Option<JsonRateLimits>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonRateLimits {
    pub minute_limit: Option<usize>,
    pub day_limit: Option<usize>,
}
