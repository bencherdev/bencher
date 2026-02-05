use std::collections::HashMap;

use bencher_valid::{DateTime, ImageDigest, Url};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::RunnerUuid;
use crate::ProjectUuid;

crate::typed_uuid::typed_uuid!(JobUuid);

/// A list of jobs
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonJobs(pub Vec<JsonJob>);

crate::from_vec!(JsonJobs[JsonJob]);

/// A benchmark job
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonJob {
    pub uuid: JobUuid,
    pub status: JobStatus,
    /// Job specification (only included when claimed by a runner)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spec: Option<JsonJobSpec>,
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
    /// Maximum time to wait for a job (long-poll), in seconds. Max 60 (default 30)
    pub poll_timeout: Option<u32>,
}

/// Job specification sent to runners.
///
/// Contains the minimal information needed for a runner to execute a job.
/// Designed to minimize data leakage - runners only learn what's necessary
/// to pull and execute an OCI image.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonJobSpec {
    /// Registry URL for pulling the OCI image (e.g., `https://registry.bencher.dev`)
    pub registry: Url,
    /// Project UUID for OCI authentication scoping
    pub project: ProjectUuid,
    /// Image digest - must be immutable (e.g., "sha256:abc123...")
    pub digest: ImageDigest,
    /// Entrypoint override (like Docker ENTRYPOINT)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entrypoint: Option<Vec<String>>,
    /// Command override (like Docker CMD)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cmd: Option<Vec<String>>,
    /// Environment variables passed to the container
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
    /// Number of virtual CPUs for the VM
    pub vcpu: u32,
    /// Memory size in bytes
    pub memory: u64,
    /// Disk size in bytes
    pub disk: u64,
    /// Maximum execution time in seconds
    pub timeout: u32,
    /// Whether the VM has network access
    pub network: bool,
}

const PENDING_INT: i32 = 0;
const CLAIMED_INT: i32 = 1;
const RUNNING_INT: i32 = 2;
const COMPLETED_INT: i32 = 3;
const FAILED_INT: i32 = 4;
const CANCELED_INT: i32 = 5;

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
    Pending = PENDING_INT,
    Claimed = CLAIMED_INT,
    Running = RUNNING_INT,
    Completed = COMPLETED_INT,
    Failed = FAILED_INT,
    Canceled = CANCELED_INT,
}

impl JobStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Canceled)
    }
}

#[cfg(feature = "db")]
mod job_status_db {
    use super::{
        CANCELED_INT, CLAIMED_INT, COMPLETED_INT, FAILED_INT, JobStatus, PENDING_INT, RUNNING_INT,
    };

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
