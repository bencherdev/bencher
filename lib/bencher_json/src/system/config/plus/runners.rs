#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Default heartbeat timeout: 90 seconds
pub const DEFAULT_HEARTBEAT_TIMEOUT_SECS: u64 = 90;

/// Default job timeout grace period: 60 seconds
pub const DEFAULT_JOB_TIMEOUT_GRACE_PERIOD_SECS: u64 = 60;

/// Runner system configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonRunners {
    /// Time in seconds without a heartbeat before marking a job as failed.
    /// Defaults to 90 seconds.
    #[serde(default = "default_heartbeat_timeout")]
    pub heartbeat_timeout: u64,
    /// Extra time in seconds beyond a job's configured timeout before the server cancels it.
    /// Defaults to 60 seconds.
    #[serde(default = "default_job_timeout_grace_period")]
    pub job_timeout_grace_period: u64,
}

fn default_heartbeat_timeout() -> u64 {
    DEFAULT_HEARTBEAT_TIMEOUT_SECS
}

fn default_job_timeout_grace_period() -> u64 {
    DEFAULT_JOB_TIMEOUT_GRACE_PERIOD_SECS
}
