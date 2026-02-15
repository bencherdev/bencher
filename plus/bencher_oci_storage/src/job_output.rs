//! Job output storage for persisting runner job output.
//!
//! Stores a single JSON blob per job containing stdout, stderr, output files,
//! exit code, and error information. Data is stored at the path:
//! `{project_uuid}/output/v0/jobs/{job_uuid}`

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
        match self.0 {
            OciStorage::S3(s3) => s3.put_job_output(project, job, output).await,
            OciStorage::Local(local) => local.put_job_output(project, job, output).await,
        }
    }

    /// Retrieve job output for a job, or `None` if not stored.
    pub async fn get(
        &self,
        project: ProjectUuid,
        job: JobUuid,
    ) -> Result<Option<JsonJobOutput>, OciStorageError> {
        match self.0 {
            OciStorage::S3(s3) => s3.get_job_output(project, job).await,
            OciStorage::Local(local) => local.get_job_output(project, job).await,
        }
    }
}
