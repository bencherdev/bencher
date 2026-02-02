use bencher_valid::DateTime;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::RunnerUuid;

crate::typed_uuid::typed_uuid!(JobUuid);

/// Job status
#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Integer))]
#[serde(rename_all = "snake_case")]
#[repr(i32)]
pub enum JobStatus {
    #[default]
    Pending = 0,
    Claimed = 1,
    Running = 2,
    Completed = 3,
    Failed = 4,
    Canceled = 5,
}

impl JobStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Canceled)
    }
}

#[cfg(feature = "db")]
mod job_status_db {
    use super::JobStatus;

    const PENDING_INT: i32 = 0;
    const CLAIMED_INT: i32 = 1;
    const RUNNING_INT: i32 = 2;
    const COMPLETED_INT: i32 = 3;
    const FAILED_INT: i32 = 4;
    const CANCELED_INT: i32 = 5;

    #[derive(Debug, thiserror::Error)]
    pub enum JobStatusError {
        #[error("Invalid job status value: {0}")]
        Invalid(i32),
    }

    impl<DB> diesel::serialize::ToSql<diesel::sql_types::Integer, DB> for JobStatus
    where
        DB: diesel::backend::Backend,
        i32: diesel::serialize::ToSql<diesel::sql_types::Integer, DB>,
    {
        fn to_sql<'b>(
            &'b self,
            out: &mut diesel::serialize::Output<'b, '_, DB>,
        ) -> diesel::serialize::Result {
            match self {
                Self::Pending => PENDING_INT.to_sql(out),
                Self::Claimed => CLAIMED_INT.to_sql(out),
                Self::Running => RUNNING_INT.to_sql(out),
                Self::Completed => COMPLETED_INT.to_sql(out),
                Self::Failed => FAILED_INT.to_sql(out),
                Self::Canceled => CANCELED_INT.to_sql(out),
            }
        }
    }

    impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Integer, DB> for JobStatus
    where
        DB: diesel::backend::Backend,
        i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, DB>,
    {
        fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            match i32::from_sql(bytes)? {
                PENDING_INT => Ok(Self::Pending),
                CLAIMED_INT => Ok(Self::Claimed),
                RUNNING_INT => Ok(Self::Running),
                COMPLETED_INT => Ok(Self::Completed),
                FAILED_INT => Ok(Self::Failed),
                CANCELED_INT => Ok(Self::Canceled),
                value => Err(Box::new(JobStatusError::Invalid(value))),
            }
        }
    }
}

/// A benchmark job
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonJob {
    pub uuid: JobUuid,
    pub status: JobStatus,
    pub runner: Option<RunnerUuid>,
    pub claimed: Option<DateTime>,
    pub started: Option<DateTime>,
    pub completed: Option<DateTime>,
    pub exit_code: Option<i32>,
    pub created: DateTime,
    pub modified: DateTime,
}

/// Update job status (runner agent endpoint)
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUpdateJob {
    /// New job status (running, completed, failed)
    pub status: JobStatus,
    /// Exit code (required for completed/failed)
    pub exit_code: Option<i32>,
    /// Standard output
    pub stdout: Option<String>,
    /// Standard error
    pub stderr: Option<String>,
    /// Combined or additional output
    pub output: Option<String>,
}

/// Response to job update
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUpdateJobResponse {
    /// If true, job was canceled - runner should stop execution
    pub canceled: bool,
}

/// Request to claim a job (runner agent endpoint)
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonClaimJob {
    /// Maximum time to wait for a job (long-poll), in seconds. Max 60.
    #[serde(default = "default_poll_timeout")]
    pub poll_timeout: u32,
}

fn default_poll_timeout() -> u32 {
    30
}

/// A list of jobs
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonJobs(pub Vec<JsonJob>);

crate::from_vec!(JsonJobs[JsonJob]);
