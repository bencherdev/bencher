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
    pub request: Option<JsonRequestRateLimiter>,
    pub auth: Option<JsonAuthRateLimiter>,
    pub unclaimed: Option<JsonUnclaimedRateLimiter>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonRequestRateLimiter {
    pub public: Option<JsonRateLimits>,
    pub user: Option<JsonRateLimits>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonAuthRateLimiter {
    pub attempt: Option<JsonRateLimits>,
    pub invite: Option<JsonRateLimits>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUnclaimedRateLimiter {
    pub run: Option<JsonRateLimits>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonRateLimits {
    pub minute: Option<usize>,
    pub hour: Option<usize>,
    pub day: Option<usize>,
}
