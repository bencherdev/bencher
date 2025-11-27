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
    // Requests
    pub pub_requests_per_minute_limit: Option<u32>,
    pub user_requests_per_minute_limit: Option<u32>,
}
