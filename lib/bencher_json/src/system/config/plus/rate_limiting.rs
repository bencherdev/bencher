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
    pub email: Option<JsonEmailRateLimiter>,
    pub request: Option<JsonRequestRateLimiter>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonEmailRateLimiter {
    pub auth: Option<JsonRateLimits>,
    pub invite: Option<JsonRateLimits>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonRequestRateLimiter {
    pub public: Option<JsonRateLimits>,
    pub user: Option<JsonRateLimits>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonRateLimits {
    pub minute_limit: Option<usize>,
    pub day_limit: Option<usize>,
}
