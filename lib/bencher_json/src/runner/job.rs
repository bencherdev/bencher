use std::collections::HashMap;

use bencher_valid::{DateTime, ImageDigest, Timeout, Url};
use camino::Utf8PathBuf;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::RunnerUuid;
use super::spec::JsonSpec;
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
    /// Resource spec for this job
    pub spec: JsonSpec,
    /// Job configuration (only included when claimed by a runner)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<JsonJobConfig>,
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
    /// File path to contents map
    #[typeshare(typescript(type = "Record<string, string> | undefined"))]
    #[cfg_attr(feature = "schema", schemars(with = "Option<HashMap<String, String>>"))]
    pub output: Option<HashMap<Utf8PathBuf, String>>,
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

/// Job configuration sent to runners.
///
/// Contains the execution details needed for a runner to execute a job.
/// Designed to minimize data leakage - runners only learn what's necessary
/// to pull and execute an OCI image. Resource requirements (cpu, memory,
/// disk, network) are in the associated spec.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonJobConfig {
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
    /// Maximum execution time in seconds
    pub timeout: Timeout,
    /// File paths to read from the VM after job completion
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<Vec<String>>"))]
    pub file_paths: Option<Vec<Utf8PathBuf>>,
}

const UNCLAIMED_PRIORITY_INT: i32 = 0;
const FREE_PRIORITY_INT: i32 = 100;
const TEAM_PRIORITY_INT: i32 = 200;
const ENTERPRISE_PRIORITY_INT: i32 = 300;

/// Job priority — determines scheduling order and concurrency limits.
///
/// Priority tiers:
/// - Enterprise (300): Unlimited concurrent jobs
/// - Team (200): Unlimited concurrent jobs
/// - Free (100): 1 concurrent job per organization
/// - Unclaimed (0): 1 concurrent job per source IP
#[typeshare::typeshare]
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Integer))]
#[serde(rename_all = "snake_case")]
#[repr(i32)]
pub enum JobPriority {
    #[default]
    Unclaimed = UNCLAIMED_PRIORITY_INT,
    Free = FREE_PRIORITY_INT,
    Team = TEAM_PRIORITY_INT,
    Enterprise = ENTERPRISE_PRIORITY_INT,
}

impl JobPriority {
    /// Returns true if this priority tier has unlimited concurrency.
    pub fn is_unlimited(&self) -> bool {
        matches!(self, Self::Team | Self::Enterprise)
    }

    /// Returns true if this priority is in the Free tier.
    pub fn is_free(&self) -> bool {
        matches!(self, Self::Free)
    }
}

impl From<JobPriority> for i32 {
    fn from(priority: JobPriority) -> Self {
        priority as Self
    }
}

#[cfg(feature = "db")]
mod job_priority_db {
    use super::{
        ENTERPRISE_PRIORITY_INT, FREE_PRIORITY_INT, JobPriority, TEAM_PRIORITY_INT,
        UNCLAIMED_PRIORITY_INT,
    };

    #[derive(Debug, thiserror::Error)]
    pub enum JobPriorityError {
        #[error("Invalid job priority value: {0}")]
        Invalid(i32),
    }

    impl<DB> diesel::serialize::ToSql<diesel::sql_types::Integer, DB> for JobPriority
    where
        DB: diesel::backend::Backend,
        i32: diesel::serialize::ToSql<diesel::sql_types::Integer, DB>,
    {
        fn to_sql<'b>(
            &'b self,
            out: &mut diesel::serialize::Output<'b, '_, DB>,
        ) -> diesel::serialize::Result {
            match self {
                Self::Unclaimed => UNCLAIMED_PRIORITY_INT.to_sql(out),
                Self::Free => FREE_PRIORITY_INT.to_sql(out),
                Self::Team => TEAM_PRIORITY_INT.to_sql(out),
                Self::Enterprise => ENTERPRISE_PRIORITY_INT.to_sql(out),
            }
        }
    }

    impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Integer, DB> for JobPriority
    where
        DB: diesel::backend::Backend,
        i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, DB>,
    {
        fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            match i32::from_sql(bytes)? {
                UNCLAIMED_PRIORITY_INT => Ok(Self::Unclaimed),
                FREE_PRIORITY_INT => Ok(Self::Free),
                TEAM_PRIORITY_INT => Ok(Self::Team),
                ENTERPRISE_PRIORITY_INT => Ok(Self::Enterprise),
                value => Err(Box::new(JobPriorityError::Invalid(value))),
            }
        }
    }
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
