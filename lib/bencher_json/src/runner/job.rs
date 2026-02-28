use std::collections::{BTreeMap, HashMap};

use bencher_valid::{DateTime, ImageDigest, ImageReference, Jwt, PollTimeout, Timeout, Url};
use camino::Utf8PathBuf;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::RunnerUuid;
use super::job_status::JobStatus;
use crate::ProjectUuid;
use crate::project::report::{Iteration, JsonAverage, JsonFold};
use crate::spec::{JsonSpec, SpecResourceId};

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
    pub created: DateTime,
    pub modified: DateTime,
    /// Job output (stdout, stderr, files) from blob storage, included for terminal jobs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<JsonJobOutput>,
}

/// Output from a single benchmark iteration.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonIterationOutput {
    /// Exit code from the benchmark command
    pub exit_code: i32,
    /// Standard output from the benchmark
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout: Option<String>,
    /// Standard error from the benchmark
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,
    /// File path to contents map
    #[serde(skip_serializing_if = "Option::is_none")]
    #[typeshare(typescript(type = "Record<string, string> | undefined"))]
    #[cfg_attr(feature = "schema", schemars(with = "Option<HashMap<String, String>>"))]
    pub output: Option<BTreeMap<Utf8PathBuf, String>>,
}

/// Job output stored in blob storage after job completion or failure.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonJobOutput {
    /// Per-iteration results
    pub results: Vec<JsonIterationOutput>,
    /// Error message (present when the job failed).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Unchecked deserialization target for `JsonNewRunJob`.
#[derive(Deserialize)]
struct JsonUncheckedNewRunJob {
    pub image: ImageReference,
    pub spec: Option<SpecResourceId>,
    pub entrypoint: Option<Vec<String>>,
    pub cmd: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub timeout: Option<Timeout>,
    pub file_paths: Option<Vec<Utf8PathBuf>>,
    pub build_time: Option<bool>,
    pub file_size: Option<bool>,
    pub iter: Option<Iteration>,
    pub allow_failure: Option<bool>,
    pub backdate: Option<DateTime>,
}

impl TryFrom<JsonUncheckedNewRunJob> for JsonNewRunJob {
    type Error = JobConfigError;

    fn try_from(unchecked: JsonUncheckedNewRunJob) -> Result<Self, Self::Error> {
        let JsonUncheckedNewRunJob {
            image,
            spec,
            entrypoint,
            cmd,
            env,
            timeout,
            file_paths,
            build_time,
            file_size,
            iter,
            allow_failure,
            backdate,
        } = unchecked;
        validate_collection_sizes(
            entrypoint.as_ref(),
            cmd.as_ref(),
            file_paths.as_ref(),
            env.as_ref(),
        )?;
        Ok(JsonNewRunJob {
            image,
            spec,
            entrypoint,
            cmd,
            env,
            timeout,
            file_paths,
            build_time,
            file_size,
            iter,
            allow_failure,
            backdate,
        })
    }
}

/// Job configuration for a remote runner execution.
///
/// Sent as part of `JsonNewRun` when the CLI `--image` flag is used.
/// The API server uses this to create a job for a bare metal runner
/// instead of expecting locally-executed benchmark results.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "JsonUncheckedNewRunJob")]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewRunJob {
    /// OCI image reference (e.g. "alpine:3.18", "ghcr.io/owner/repo:v1", "image@sha256:abc...")
    pub image: ImageReference,
    /// Hardware spec slug or UUID to run on
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spec: Option<SpecResourceId>,
    /// Container entrypoint override (like Docker ENTRYPOINT)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entrypoint: Option<Vec<String>>,
    /// Command override (like Docker CMD)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cmd: Option<Vec<String>>,
    /// Environment variables passed to the container
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
    /// Maximum execution time in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<Timeout>,
    /// File paths to collect from the VM after job completion
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<Vec<String>>"))]
    pub file_paths: Option<Vec<Utf8PathBuf>>,
    /// Track the build time of the benchmark command
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_time: Option<bool>,
    /// Track the file size of the output files instead of parsing their contents
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_size: Option<bool>,
    /// Number of benchmark iterations for the runner to execute
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iter: Option<Iteration>,
    /// Allow benchmark failure without short-circuiting iterations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_failure: Option<bool>,
    /// Backdate the report start time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backdate: Option<DateTime>,
}

/// Default poll timeout in seconds for job claiming long-poll.
pub const DEFAULT_POLL_TIMEOUT: u32 = 30;
/// Minimum poll timeout in seconds.
pub const MIN_POLL_TIMEOUT: u32 = 1;
/// Maximum poll timeout in seconds.
pub const MAX_POLL_TIMEOUT: u32 = 900;

pub use crate::{MAX_CMD_LEN, MAX_ENTRYPOINT_LEN, MAX_ENV_LEN, MAX_FILE_PATHS_LEN};

/// Request to claim a job (runner agent endpoint)
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonClaimJob {
    /// Maximum time to wait for a job (long-poll), in seconds (1-900)
    pub poll_timeout: Option<PollTimeout>,
}

/// A claimed job returned to the runner agent.
///
/// Standalone type containing everything a runner needs to execute a job.
/// Config and OCI token are always present (not Optional) since
/// they are guaranteed at claim time.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonClaimedJob {
    pub uuid: JobUuid,
    /// Full spec details (architecture, cpu, memory, etc.)
    pub spec: JsonSpec,
    /// Execution config — always present for claimed jobs
    pub config: JsonJobConfig,
    /// Short-lived, project-scoped OCI pull token
    pub oci_token: Jwt,
    /// Maximum execution time in seconds
    pub timeout: Timeout,
    /// Job creation timestamp
    pub created: DateTime,
}

/// Job configuration validation errors.
#[derive(Debug, thiserror::Error)]
pub enum JobConfigError {
    #[error("entrypoint length {0} exceeds maximum {MAX_ENTRYPOINT_LEN}")]
    EntrypointTooLong(usize),
    #[error("cmd length {0} exceeds maximum {MAX_CMD_LEN}")]
    CmdTooLong(usize),
    #[error("file_paths length {0} exceeds maximum {MAX_FILE_PATHS_LEN}")]
    FilePathsTooLong(usize),
    #[error("env length {0} exceeds maximum {MAX_ENV_LEN}")]
    EnvTooLong(usize),
}

fn validate_collection_sizes(
    entrypoint: Option<&Vec<String>>,
    cmd: Option<&Vec<String>>,
    file_paths: Option<&Vec<Utf8PathBuf>>,
    env: Option<&HashMap<String, String>>,
) -> Result<(), JobConfigError> {
    if let Some(entrypoint) = entrypoint
        && entrypoint.len() > MAX_ENTRYPOINT_LEN
    {
        return Err(JobConfigError::EntrypointTooLong(entrypoint.len()));
    }
    if let Some(cmd) = cmd
        && cmd.len() > MAX_CMD_LEN
    {
        return Err(JobConfigError::CmdTooLong(cmd.len()));
    }
    if let Some(file_paths) = file_paths
        && file_paths.len() > MAX_FILE_PATHS_LEN
    {
        return Err(JobConfigError::FilePathsTooLong(file_paths.len()));
    }
    if let Some(env) = env
        && env.len() > MAX_ENV_LEN
    {
        return Err(JobConfigError::EnvTooLong(env.len()));
    }
    Ok(())
}

#[derive(Deserialize)]
struct JsonUncheckedJobConfig {
    pub registry: Url,
    pub project: ProjectUuid,
    pub digest: ImageDigest,
    pub entrypoint: Option<Vec<String>>,
    pub cmd: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub timeout: Timeout,
    pub file_paths: Option<Vec<Utf8PathBuf>>,
    pub average: Option<JsonAverage>,
    pub iter: Option<Iteration>,
    pub fold: Option<JsonFold>,
    pub allow_failure: Option<bool>,
    pub backdate: Option<DateTime>,
}

impl TryFrom<JsonUncheckedJobConfig> for JsonJobConfig {
    type Error = JobConfigError;

    fn try_from(unchecked: JsonUncheckedJobConfig) -> Result<Self, Self::Error> {
        let JsonUncheckedJobConfig {
            registry,
            project,
            digest,
            entrypoint,
            cmd,
            env,
            timeout,
            file_paths,
            average,
            iter,
            fold,
            allow_failure,
            backdate,
        } = unchecked;
        validate_collection_sizes(
            entrypoint.as_ref(),
            cmd.as_ref(),
            file_paths.as_ref(),
            env.as_ref(),
        )?;
        Ok(JsonJobConfig {
            registry,
            project,
            digest,
            entrypoint,
            cmd,
            env,
            timeout,
            file_paths,
            average,
            iter,
            fold,
            allow_failure,
            backdate,
        })
    }
}

/// Job configuration sent to runners.
///
/// Contains the execution details needed for a runner to execute a job.
/// Designed to minimize data leakage - runners only learn what's necessary
/// to pull and execute an OCI image. Resource requirements (cpu, memory,
/// disk, network) are in the associated spec.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "JsonUncheckedJobConfig")]
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
    /// Benchmark harness suggested central tendency
    #[serde(skip_serializing_if = "Option::is_none")]
    pub average: Option<JsonAverage>,
    /// Number of benchmark iterations for the runner to execute
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iter: Option<Iteration>,
    /// Fold operation for combining multiple iteration results
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fold: Option<JsonFold>,
    /// Allow benchmark failure without short-circuiting iterations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_failure: Option<bool>,
    /// Backdate the report start time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backdate: Option<DateTime>,
}

#[cfg(test)]
#[expect(clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_all_fields() {
        let mut output_files = BTreeMap::new();
        output_files.insert(Utf8PathBuf::from("/tmp/results.json"), "{}".to_owned());

        let original = JsonJobOutput {
            results: vec![JsonIterationOutput {
                exit_code: 0,
                stdout: Some("hello".into()),
                stderr: Some("world".into()),
                output: Some(output_files),
            }],
            error: Some("oops".into()),
        };

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: JsonJobOutput = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.results.len(), 1);
        assert_eq!(deserialized.results[0].exit_code, 0);
        assert_eq!(deserialized.results[0].stdout.as_deref(), Some("hello"));
        assert_eq!(deserialized.results[0].stderr.as_deref(), Some("world"));
        assert_eq!(deserialized.results[0].output.as_ref().unwrap().len(), 1);
        assert_eq!(deserialized.error.as_deref(), Some("oops"));
    }

    #[test]
    fn round_trip_completed_shape() {
        let original = JsonJobOutput {
            results: vec![JsonIterationOutput {
                exit_code: 0,
                stdout: Some("stdout".into()),
                stderr: Some("stderr".into()),
                output: Some(BTreeMap::new()),
            }],
            error: None,
        };

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: JsonJobOutput = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.results.len(), 1);
        assert_eq!(deserialized.results[0].exit_code, 0);
        assert!(deserialized.error.is_none());
        assert_eq!(deserialized.results[0].stdout.as_deref(), Some("stdout"));
        assert_eq!(deserialized.results[0].stderr.as_deref(), Some("stderr"));
    }

    #[test]
    fn round_trip_failed_shape() {
        let original = JsonJobOutput {
            results: vec![JsonIterationOutput {
                exit_code: 1,
                stdout: Some("stdout".into()),
                stderr: Some("stderr".into()),
                output: None,
            }],
            error: Some("something broke".into()),
        };

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: JsonJobOutput = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.results.len(), 1);
        assert_eq!(deserialized.results[0].exit_code, 1);
        assert_eq!(deserialized.error.as_deref(), Some("something broke"));
        assert!(deserialized.results[0].output.is_none());
    }

    #[test]
    fn round_trip_minimal() {
        let original = JsonJobOutput {
            results: Vec::new(),
            error: None,
        };

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: JsonJobOutput = serde_json::from_str(&json).unwrap();

        assert!(deserialized.results.is_empty());
        assert!(deserialized.error.is_none());
    }

    #[test]
    fn round_trip_with_output_files() {
        let mut files = BTreeMap::new();
        files.insert(
            Utf8PathBuf::from("/tmp/results.json"),
            r#"{"metric": 42}"#.to_owned(),
        );
        files.insert(Utf8PathBuf::from("/tmp/log.txt"), "log data".to_owned());

        let original = JsonJobOutput {
            results: vec![JsonIterationOutput {
                exit_code: 0,
                stdout: None,
                stderr: None,
                output: Some(files),
            }],
            error: None,
        };

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: JsonJobOutput = serde_json::from_str(&json).unwrap();

        let output = deserialized.results[0].output.as_ref().unwrap();
        assert_eq!(output.len(), 2);
        assert_eq!(
            output.get(Utf8PathBuf::from("/tmp/results.json").as_path()),
            Some(&r#"{"metric": 42}"#.to_owned())
        );
    }

    #[test]
    fn round_trip_multiple_iterations() {
        let original = JsonJobOutput {
            results: vec![
                JsonIterationOutput {
                    exit_code: 0,
                    stdout: Some("iter1".into()),
                    stderr: None,
                    output: None,
                },
                JsonIterationOutput {
                    exit_code: 0,
                    stdout: Some("iter2".into()),
                    stderr: None,
                    output: None,
                },
            ],
            error: None,
        };

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: JsonJobOutput = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.results.len(), 2);
        assert_eq!(deserialized.results[0].stdout.as_deref(), Some("iter1"));
        assert_eq!(deserialized.results[1].stdout.as_deref(), Some("iter2"));
    }

    fn job_config_json(
        entrypoint_len: usize,
        cmd_len: usize,
        file_paths_len: usize,
        env_len: usize,
    ) -> String {
        let entrypoint: Vec<String> = (0..entrypoint_len).map(|i| format!("arg{i}")).collect();
        let cmd: Vec<String> = (0..cmd_len).map(|i| format!("cmd{i}")).collect();
        let file_paths: Vec<String> = (0..file_paths_len).map(|i| format!("/f/{i}")).collect();
        let env: HashMap<String, String> = (0..env_len)
            .map(|i| (format!("KEY{i}"), format!("val{i}")))
            .collect();
        format!(
            r#"{{"registry":"https://registry.bencher.dev","project":"00000000-0000-0000-0000-000000000000","digest":"sha256:{digest}","entrypoint":{entrypoint},"cmd":{cmd},"env":{env},"timeout":300,"file_paths":{file_paths}}}"#,
            digest = "a".repeat(64),
            entrypoint = serde_json::to_string(&entrypoint).unwrap(),
            cmd = serde_json::to_string(&cmd).unwrap(),
            file_paths = serde_json::to_string(&file_paths).unwrap(),
            env = serde_json::to_string(&env).unwrap(),
        )
    }

    #[test]
    fn deserialize_entrypoint_too_long() {
        let json = job_config_json(MAX_ENTRYPOINT_LEN + 1, 0, 0, 0);
        let result = serde_json::from_str::<JsonJobConfig>(&json);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("entrypoint length"), "{err}");
    }

    #[test]
    fn deserialize_cmd_too_long() {
        let json = job_config_json(0, MAX_CMD_LEN + 1, 0, 0);
        let result = serde_json::from_str::<JsonJobConfig>(&json);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("cmd length"), "{err}");
    }

    #[test]
    fn deserialize_file_paths_too_long() {
        let json = job_config_json(0, 0, MAX_FILE_PATHS_LEN + 1, 0);
        let result = serde_json::from_str::<JsonJobConfig>(&json);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("file_paths length"), "{err}");
    }

    #[test]
    fn deserialize_env_too_long() {
        let json = job_config_json(0, 0, 0, MAX_ENV_LEN + 1);
        let result = serde_json::from_str::<JsonJobConfig>(&json);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("env length"), "{err}");
    }

    #[test]
    fn deserialize_at_max_boundary() {
        let json = job_config_json(
            MAX_ENTRYPOINT_LEN,
            MAX_CMD_LEN,
            MAX_FILE_PATHS_LEN,
            MAX_ENV_LEN,
        );
        let config: JsonJobConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(
            config.entrypoint.as_ref().unwrap().len(),
            MAX_ENTRYPOINT_LEN
        );
        assert_eq!(config.cmd.as_ref().unwrap().len(), MAX_CMD_LEN);
        assert_eq!(
            config.file_paths.as_ref().unwrap().len(),
            MAX_FILE_PATHS_LEN
        );
        assert_eq!(config.env.as_ref().unwrap().len(), MAX_ENV_LEN);
    }

    #[test]
    fn deserialize_env_at_max_boundary() {
        let json = job_config_json(0, 0, 0, MAX_ENV_LEN);
        let config: JsonJobConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.env.as_ref().unwrap().len(), MAX_ENV_LEN);
    }

    // --- JsonNewRunJob validation tests ---

    fn new_run_job_json(
        entrypoint_len: usize,
        cmd_len: usize,
        file_paths_len: usize,
        env_len: usize,
    ) -> String {
        let entrypoint: Vec<String> = (0..entrypoint_len).map(|i| format!("arg{i}")).collect();
        let cmd: Vec<String> = (0..cmd_len).map(|i| format!("cmd{i}")).collect();
        let file_paths: Vec<String> = (0..file_paths_len).map(|i| format!("/f/{i}")).collect();
        let env: HashMap<String, String> = (0..env_len)
            .map(|i| (format!("KEY{i}"), format!("val{i}")))
            .collect();
        format!(
            r#"{{"image":"ghcr.io/owner/my-image:latest","entrypoint":{entrypoint},"cmd":{cmd},"env":{env},"file_paths":{file_paths}}}"#,
            entrypoint = serde_json::to_string(&entrypoint).unwrap(),
            cmd = serde_json::to_string(&cmd).unwrap(),
            file_paths = serde_json::to_string(&file_paths).unwrap(),
            env = serde_json::to_string(&env).unwrap(),
        )
    }

    #[test]
    fn new_run_job_entrypoint_too_long() {
        let json = new_run_job_json(MAX_ENTRYPOINT_LEN + 1, 0, 0, 0);
        let result = serde_json::from_str::<JsonNewRunJob>(&json);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("entrypoint length"), "{err}");
    }

    #[test]
    fn new_run_job_cmd_too_long() {
        let json = new_run_job_json(0, MAX_CMD_LEN + 1, 0, 0);
        let result = serde_json::from_str::<JsonNewRunJob>(&json);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("cmd length"), "{err}");
    }

    #[test]
    fn new_run_job_file_paths_too_long() {
        let json = new_run_job_json(0, 0, MAX_FILE_PATHS_LEN + 1, 0);
        let result = serde_json::from_str::<JsonNewRunJob>(&json);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("file_paths length"), "{err}");
    }

    #[test]
    fn new_run_job_env_too_long() {
        let json = new_run_job_json(0, 0, 0, MAX_ENV_LEN + 1);
        let result = serde_json::from_str::<JsonNewRunJob>(&json);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("env length"), "{err}");
    }

    #[test]
    fn new_run_job_at_max_boundary() {
        let json = new_run_job_json(
            MAX_ENTRYPOINT_LEN,
            MAX_CMD_LEN,
            MAX_FILE_PATHS_LEN,
            MAX_ENV_LEN,
        );
        let job: JsonNewRunJob = serde_json::from_str(&json).unwrap();
        assert_eq!(job.entrypoint.as_ref().unwrap().len(), MAX_ENTRYPOINT_LEN);
        assert_eq!(job.cmd.as_ref().unwrap().len(), MAX_CMD_LEN);
        assert_eq!(job.file_paths.as_ref().unwrap().len(), MAX_FILE_PATHS_LEN);
        assert_eq!(job.env.as_ref().unwrap().len(), MAX_ENV_LEN);
    }

    #[test]
    fn new_run_job_env_at_max_boundary() {
        let json = new_run_job_json(0, 0, 0, MAX_ENV_LEN);
        let job: JsonNewRunJob = serde_json::from_str(&json).unwrap();
        assert_eq!(job.env.as_ref().unwrap().len(), MAX_ENV_LEN);
    }

    // --- Backwards compatibility: deserialize known JSON strings ---

    #[test]
    fn backwards_compat_completed_json() {
        let json = r#"{"results":[{"exit_code":0,"stdout":"hello","stderr":"world","output":{"/tmp/f.txt":"data"}}]}"#;
        let output: JsonJobOutput = serde_json::from_str(json).unwrap();
        assert_eq!(output.results.len(), 1);
        assert_eq!(output.results[0].exit_code, 0);
        assert_eq!(output.results[0].stdout.as_deref(), Some("hello"));
        assert_eq!(output.results[0].stderr.as_deref(), Some("world"));
        assert_eq!(
            output.results[0]
                .output
                .as_ref()
                .unwrap()
                .get(Utf8PathBuf::from("/tmp/f.txt").as_path()),
            Some(&"data".to_owned())
        );
        assert!(output.error.is_none());
    }

    #[test]
    fn backwards_compat_failed_json() {
        let json = r#"{"results":[{"exit_code":1,"stdout":"out","stderr":"err"}],"error":"something broke"}"#;
        let output: JsonJobOutput = serde_json::from_str(json).unwrap();
        assert_eq!(output.results.len(), 1);
        assert_eq!(output.results[0].exit_code, 1);
        assert_eq!(output.results[0].stdout.as_deref(), Some("out"));
        assert_eq!(output.results[0].stderr.as_deref(), Some("err"));
        assert_eq!(output.error.as_deref(), Some("something broke"));
    }

    #[test]
    fn backwards_compat_empty_results() {
        let json = r#"{"results":[]}"#;
        let output: JsonJobOutput = serde_json::from_str(json).unwrap();
        assert!(output.results.is_empty());
        assert!(output.error.is_none());
    }

    #[test]
    fn backwards_compat_file_only_output() {
        let json =
            r#"{"results":[{"exit_code":0,"output":{"/tmp/results.json":"{\"metric\":42}"}}]}"#;
        let output: JsonJobOutput = serde_json::from_str(json).unwrap();
        assert_eq!(output.results.len(), 1);
        assert_eq!(output.results[0].exit_code, 0);
        assert!(output.results[0].stdout.is_none());
        assert!(output.results[0].stderr.is_none());
        let files = output.results[0].output.as_ref().unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(
            files.get(Utf8PathBuf::from("/tmp/results.json").as_path()),
            Some(&r#"{"metric":42}"#.to_owned())
        );
    }
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
