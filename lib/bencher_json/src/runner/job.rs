use std::collections::HashMap;

use bencher_valid::{DateTime, ImageDigest, PollTimeout, Timeout, Url};
use camino::Utf8PathBuf;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::RunnerUuid;
use super::job_status::{JobStatus, JobUpdateStatus};
use crate::ProjectUuid;
use crate::spec::JsonSpec;

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
    /// Job output (stdout, stderr, files) from blob storage, included for terminal jobs.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[typeshare(typescript(type = "JsonJobOutputFailed | JsonJobOutputCompleted | undefined"))]
    pub output: Option<JsonJobOutput>,
}

/// Update job status (runner agent endpoint)
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUpdateJob {
    /// New job status (running, completed, failed)
    pub status: JobUpdateStatus,
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

/// Job output stored in blob storage after job completion or failure.
///
/// Deserialization tries `Failed` first (which has a required `error` field),
/// then falls back to `Completed`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(untagged)]
pub enum JsonJobOutput {
    /// Output from a failed job (tried first — has required `error` field as discriminator)
    Failed(JsonJobOutputFailed),
    /// Output from a completed job
    Completed(JsonJobOutputCompleted),
}

impl JsonJobOutput {
    pub fn exit_code(&self) -> Option<i32> {
        match self {
            Self::Completed(c) => Some(c.exit_code),
            Self::Failed(f) => f.exit_code,
        }
    }

    pub fn stdout(&self) -> Option<&str> {
        match self {
            Self::Completed(c) => c.stdout.as_deref(),
            Self::Failed(f) => f.stdout.as_deref(),
        }
    }

    pub fn stderr(&self) -> Option<&str> {
        match self {
            Self::Completed(c) => c.stderr.as_deref(),
            Self::Failed(f) => f.stderr.as_deref(),
        }
    }

    pub fn error(&self) -> Option<&str> {
        match self {
            Self::Completed(_) => None,
            Self::Failed(f) => Some(&f.error),
        }
    }

    pub fn output(&self) -> Option<&HashMap<Utf8PathBuf, String>> {
        match self {
            Self::Completed(c) => c.output.as_ref(),
            Self::Failed(_) => None,
        }
    }
}

/// Output from a completed job.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonJobOutputCompleted {
    pub exit_code: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,
    /// File path to contents map
    #[serde(skip_serializing_if = "Option::is_none")]
    #[typeshare(typescript(type = "Record<string, string> | undefined"))]
    #[cfg_attr(feature = "schema", schemars(with = "Option<HashMap<String, String>>"))]
    pub output: Option<HashMap<Utf8PathBuf, String>>,
}

/// Output from a failed job.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonJobOutputFailed {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,
}

/// Response to job update
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUpdateJobResponse {
    /// The current status of the job after the update
    pub status: JobStatus,
}

/// Request to claim a job (runner agent endpoint)
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonClaimJob {
    /// Maximum time to wait for a job (long-poll), in seconds (1-600)
    pub poll_timeout: Option<PollTimeout>,
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
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
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

#[cfg(feature = "db")]
mod db {
    use super::JsonJobConfig;

    impl<DB> diesel::serialize::ToSql<diesel::sql_types::Text, DB> for JsonJobConfig
    where
        DB: diesel::backend::Backend,
        for<'a> String: diesel::serialize::ToSql<diesel::sql_types::Text, DB>
            + Into<<DB::BindCollector<'a> as diesel::query_builder::BindCollector<'a, DB>>::Buffer>,
    {
        fn to_sql<'b>(
            &'b self,
            out: &mut diesel::serialize::Output<'b, '_, DB>,
        ) -> diesel::serialize::Result {
            let json = serde_json::to_string(self)?;
            out.set_value(json);
            Ok(diesel::serialize::IsNull::No)
        }
    }

    impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Text, DB> for JsonJobConfig
    where
        DB: diesel::backend::Backend,
        String: diesel::deserialize::FromSql<diesel::sql_types::Text, DB>,
    {
        fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            let json_str = String::from_sql(bytes)?;
            let config: JsonJobConfig = serde_json::from_str(&json_str)?;
            Ok(config)
        }
    }
}
