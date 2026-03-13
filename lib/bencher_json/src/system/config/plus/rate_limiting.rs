#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonRateLimiting {
    pub window: Option<u32>,
    pub unclaimed_limit: Option<u32>,
    pub claimed_limit: Option<u32>,
    pub public: Option<JsonPublicRateLimiter>,
    pub user: Option<JsonUserRateLimiter>,
    pub runner: Option<JsonRunnerRateLimiter>,
    pub oci_bandwidth: Option<JsonOciBandwidth>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPublicRateLimiter {
    pub requests: Option<JsonRateLimits>,
    pub attempts: Option<JsonRateLimits>,
    pub runs: Option<JsonRateLimits>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUserRateLimiter {
    pub requests: Option<JsonRateLimits>,
    pub attempts: Option<JsonRateLimits>,
    pub tokens: Option<JsonRateLimits>,
    pub organizations: Option<JsonRateLimits>,
    pub invites: Option<JsonRateLimits>,
    pub runs: Option<JsonRateLimits>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonRunnerRateLimiter {
    pub requests: Option<JsonRateLimits>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonRateLimits {
    pub minute: Option<usize>,
    pub hour: Option<usize>,
    pub day: Option<usize>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonOciBandwidth {
    /// Bytes per day for unclaimed organizations (0 members)
    pub unclaimed: Option<u64>,
    /// Bytes per day for free (claimed, no paid plan) organizations
    pub free: Option<u64>,
    /// Bytes per day for Plus (Team/Enterprise) organizations
    pub plus: Option<u64>,
}
