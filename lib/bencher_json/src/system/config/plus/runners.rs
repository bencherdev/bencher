use std::sync::LazyLock;

use bencher_valid::{GracePeriod, HeartbeatTimeout};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Default heartbeat timeout: 90 seconds
pub const DEFAULT_HEARTBEAT_TIMEOUT_SECS: u32 = 90;

/// Default job timeout grace period: 60 seconds
pub const DEFAULT_JOB_TIMEOUT_GRACE_PERIOD_SECS: u32 = 60;

#[expect(clippy::expect_used)]
static DEFAULT_HEARTBEAT_TIMEOUT: LazyLock<HeartbeatTimeout> = LazyLock::new(|| {
    HeartbeatTimeout::try_from(DEFAULT_HEARTBEAT_TIMEOUT_SECS)
        .expect("Default heartbeat timeout is valid")
});

#[expect(clippy::expect_used)]
static DEFAULT_JOB_TIMEOUT_GRACE_PERIOD: LazyLock<GracePeriod> = LazyLock::new(|| {
    GracePeriod::try_from(DEFAULT_JOB_TIMEOUT_GRACE_PERIOD_SECS)
        .expect("Default job timeout grace period is valid")
});

/// Runner system configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonRunners {
    /// Time in seconds without a heartbeat before marking a job as failed.
    /// Defaults to 90 seconds.
    #[serde(default = "default_heartbeat_timeout")]
    pub heartbeat_timeout: HeartbeatTimeout,
    /// Extra time in seconds beyond a job's configured timeout before the server cancels it.
    /// Defaults to 60 seconds.
    #[serde(default = "default_job_timeout_grace_period")]
    pub job_timeout_grace_period: GracePeriod,
}

fn default_heartbeat_timeout() -> HeartbeatTimeout {
    *DEFAULT_HEARTBEAT_TIMEOUT
}

fn default_job_timeout_grace_period() -> GracePeriod {
    *DEFAULT_JOB_TIMEOUT_GRACE_PERIOD
}
