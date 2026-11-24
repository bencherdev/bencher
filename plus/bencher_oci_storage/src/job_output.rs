//! Job output storage for persisting runner job output.
//!
//! Stores a single JSON blob per job containing stdout, stderr, output files,
//! exit code, and error information. Data is stored at the path:
//! `{project_uuid}/output/v0/jobs/{job_uuid}`
//!
//! Each execution of a job stores its own output as it arrives, at the path:
//! `{project_uuid}/output/v0/executions/{execution_uuid}`

use bencher_json::{JobUuid, ProjectUuid, runner::JsonJobOutput};

use crate::storage::{OciStorage, OciStorageError};

/// Thin view type for job output storage operations.
pub struct JobOutput<'a>(&'a OciStorage);

impl<'a> JobOutput<'a> {
    pub(crate) fn new(storage: &'a OciStorage) -> Self {
        Self(storage)
    }

    /// Store job output for a completed or failed job.
    pub async fn put(
        &self,
        project: ProjectUuid,
        job: JobUuid,
        output: &JsonJobOutput,
    ) -> Result<(), OciStorageError> {
        self.0.put_job_output(project, job, output).await
    }

    /// Retrieve job output for a job, or `None` if not stored.
    pub async fn get(
        &self,
        project: ProjectUuid,
        job: JobUuid,
    ) -> Result<Option<JsonJobOutput>, OciStorageError> {
        self.0.get_job_output(project, job).await
    }

    /// Store the output of one execution of a job.
    pub async fn put_execution(
        &self,
        project: ProjectUuid,
        execution: JobUuid,
        output: &JsonJobOutput,
    ) -> Result<(), OciStorageError> {
        self.0
            .put_execution_output(project, execution, output)
            .await
    }

    /// Retrieve the output of one execution of a job, or `None` if not stored.
    pub async fn get_execution(
        &self,
        project: ProjectUuid,
        execution: JobUuid,
    ) -> Result<Option<JsonJobOutput>, OciStorageError> {
        self.0.get_execution_output(project, execution).await
    }
}
