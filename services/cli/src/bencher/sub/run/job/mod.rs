use std::collections::HashMap;

use bencher_json::{JobUuid, ProjectResourceId, SpecResourceId};

use crate::parser::run::CliRunJob;

use super::RunError;

mod job_wait;

pub use job_wait::{FinishedJob, JobWait};

#[expect(
    clippy::expect_used,
    reason = "constant 5 is always a valid PollTimeout"
)]
pub static DEFAULT_POLL_INTERVAL: std::sync::LazyLock<bencher_json::PollTimeout> =
    std::sync::LazyLock::new(|| {
        bencher_json::PollTimeout::try_from(5).expect("5 is a valid PollTimeout")
    });

#[derive(Debug)]
pub enum Job {
    /// Submit a new job with `--image`.
    Submit(SubmitJob),
    /// Attach to a submitted job with `--job`.
    Attach(AttachJob),
}

#[derive(Debug)]
pub struct SubmitJob {
    pub image: bencher_json::ImageReference,
    pub spec: Option<SpecResourceId>,
    pub entrypoint: Option<String>,
    pub env: Option<HashMap<String, String>>,
    pub timeout: Option<bencher_json::Timeout>,
    pub build_time: bool,
    pub wait: JobWait,
    pub detach: bool,
}

#[derive(Debug)]
pub struct AttachJob {
    pub project: ProjectResourceId,
    pub uuid: JobUuid,
    pub wait: JobWait,
}

impl Job {
    pub fn new(
        cli_job: CliRunJob,
        project: Option<&ProjectResourceId>,
        build_time: bool,
    ) -> Result<Option<Self>, RunError> {
        let CliRunJob {
            image,
            job,
            spec,
            entrypoint,
            env,
            job_timeout,
            job_poll_interval,
            detach,
        } = cli_job;
        let poll_interval = job_poll_interval.unwrap_or(*DEFAULT_POLL_INTERVAL);
        if let Some(uuid) = job {
            return Ok(Some(Self::Attach(AttachJob {
                project: project.cloned().ok_or(RunError::JobRequiresProject)?,
                uuid,
                wait: JobWait::attach(poll_interval, job_timeout),
            })));
        }
        Ok(image.map(|image| {
            Self::Submit(SubmitJob {
                image,
                spec,
                entrypoint,
                env: env.map(bencher_parser::parse_env),
                timeout: job_timeout,
                build_time,
                wait: JobWait::submitted(poll_interval, job_timeout),
                detach,
            })
        }))
    }
}
