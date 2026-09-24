use std::time::Duration;

use bencher_json::{JobStatus, JsonJob, PollTimeout, ReportUuid, Timeout, runner::JsonJobOutput};
use tokio::time::{Instant, sleep};

use crate::{BackendError, cli_eprintln_quietable};

use super::RunError;

const DEFAULT_JOB_TIMEOUT: u64 = Timeout::PLUS_DEFAULT.as_secs();
// The wait allows for queue time on top of the job's own timeout.
const CLI_TIMEOUT_MULTIPLE: u64 = 2;

/// How `bencher run` waits for a remote job to reach a terminal state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JobWait {
    poll_interval: PollTimeout,
    budget: WaitBudget,
    first_poll: FirstPoll,
}

impl JobWait {
    /// Wait for a job this run just submitted.
    pub fn submitted(poll_interval: PollTimeout, timeout: Option<Timeout>) -> Self {
        let job_timeout = timeout.map_or(DEFAULT_JOB_TIMEOUT, Timeout::as_secs);
        Self {
            poll_interval,
            budget: WaitBudget::Secs(job_timeout.saturating_mul(CLI_TIMEOUT_MULTIPLE)),
            first_poll: FirstPoll::AfterInterval,
        }
    }

    /// Wait for a job an earlier run submitted.
    pub fn attach(poll_interval: PollTimeout, budget: Option<Timeout>) -> Self {
        Self {
            poll_interval,
            budget: budget.map_or(WaitBudget::TwiceJobTimeout, |budget| {
                WaitBudget::Secs(budget.as_secs())
            }),
            first_poll: FirstPoll::Immediate,
        }
    }

    pub async fn until_finished<F, Fut>(
        self,
        log: bool,
        mut get_job: F,
    ) -> Result<FinishedJob, RunError>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<JsonJob, BackendError>>,
    {
        let Self {
            poll_interval,
            budget,
            first_poll,
        } = self;
        let poll_interval = Duration::from_secs(u64::from(u32::from(poll_interval)));
        let start = Instant::now();
        let mut budget_secs = budget.secs();
        let mut wait = first_poll == FirstPoll::AfterInterval;
        let mut last_status: Option<JobStatus> = None;

        loop {
            if wait {
                sleep(poll_interval).await;
                if let Some(budget_secs) = budget_secs
                    && start.elapsed().as_secs() > budget_secs
                {
                    return Err(RunError::JobTimeout(budget_secs));
                }
            }
            wait = true;

            let json_job = get_job().await.map_err(RunError::PollJob)?;
            if budget_secs.is_none() {
                budget_secs = Some(
                    json_job
                        .timeout
                        .as_secs()
                        .saturating_mul(CLI_TIMEOUT_MULTIPLE),
                );
            }

            let status = json_job.status;
            // Print status changes
            if !last_status.is_some_and(|ls| ls == status) {
                cli_eprintln_quietable!(log, "Job status: {status}");
                last_status = Some(status);
            }

            if let Some(finished) = FinishedJob::new(json_job) {
                return Ok(finished);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WaitBudget {
    Secs(u64),
    /// Twice the job's own timeout, read on the first poll.
    TwiceJobTimeout,
}

impl WaitBudget {
    fn secs(self) -> Option<u64> {
        match self {
            Self::Secs(secs) => Some(secs),
            Self::TwiceJobTimeout => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FirstPoll {
    Immediate,
    AfterInterval,
}

/// A job in a terminal state.
#[derive(Debug)]
pub struct FinishedJob {
    pub report: ReportUuid,
    pub output: Option<JsonJobOutput>,
    pub result: Result<(), RunError>,
}

impl FinishedJob {
    fn new(json_job: JsonJob) -> Option<Self> {
        let JsonJob {
            report,
            status,
            output,
            ..
        } = json_job;
        let error = |default: &str| {
            output
                .as_ref()
                .and_then(|output| output.error.clone())
                .unwrap_or_else(|| default.to_owned())
        };
        let result = match status {
            JobStatus::Processed => Ok(()),
            JobStatus::Failed => Err(RunError::JobFailed(error("Unknown error"))),
            JobStatus::Canceled => Err(RunError::JobCanceled(error("Job was canceled"))),
            // Non-terminal states: keep polling.
            // `Completed` means the runner finished execution and sent results,
            // but the server hasn't finished processing them into metrics/alerts yet.
            // It transitions to `Processed` (or `Failed`) once processing completes.
            // `Unknown` means the server lost contact with the runner and is waiting to hear back.
            JobStatus::Pending
            | JobStatus::Claimed
            | JobStatus::Running
            | JobStatus::Completed
            | JobStatus::Unknown => return None,
        };
        Some(Self {
            report,
            output,
            result,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::future::{Ready, ready};

    use bencher_json::{JobStatus, JsonJob, PollTimeout, ReportUuid, Timeout};

    use super::{FinishedJob, JobWait, RunError};
    use crate::BackendError;

    const REPORT: &str = "4f6d1b2a-3c5e-4d7f-8a9b-0c1d2e3f4a5b";

    fn json_job(status: JobStatus, timeout: u32, error: Option<&str>) -> JsonJob {
        serde_json::from_value(serde_json::json!({
            "uuid": "8d2b6c4e-5f3a-4b1c-9e7d-0a1b2c3d4e5f",
            "report": REPORT,
            "status": status,
            "spec": {
                "uuid": "1a2b3c4d-5e6f-4a7b-8c9d-0e1f2a3b4c5d",
                "name": "Test Spec",
                "slug": "test-spec",
                "os": "linux",
                "architecture": "x86_64",
                "cpu": 2,
                "memory": 4096,
                "disk": 8192,
                "network": false,
                "created": "2026-01-01T00:00:00Z",
                "modified": "2026-01-01T00:00:00Z"
            },
            "timeout": timeout,
            "created": "2026-01-01T00:00:00Z",
            "modified": "2026-01-01T00:00:00Z",
            "output": error.map(|error| serde_json::json!({ "results": [], "error": error })),
        }))
        .unwrap()
    }

    /// Answer each poll with the next status, repeating the last one, and count the polls.
    fn job_statuses<'a>(
        statuses: &'a [JobStatus],
        timeout: u32,
        polls: &'a mut usize,
    ) -> impl FnMut() -> Ready<Result<JsonJob, BackendError>> + 'a {
        move || {
            let status = statuses
                .get(*polls)
                .or_else(|| statuses.last())
                .copied()
                .unwrap();
            *polls += 1;
            ready(Ok(json_job(status, timeout, None)))
        }
    }

    fn poll_interval(secs: u32) -> PollTimeout {
        PollTimeout::try_from(secs).unwrap()
    }

    fn timeout(secs: u32) -> Timeout {
        Timeout::try_from(secs).unwrap()
    }

    #[tokio::test(start_paused = true)]
    async fn attach_terminal_job_returns_without_sleeping() {
        let start = tokio::time::Instant::now();
        let mut polls = 0;
        let finished = JobWait::attach(poll_interval(5), None)
            .until_finished(false, job_statuses(&[JobStatus::Processed], 3, &mut polls))
            .await
            .unwrap();
        assert_eq!(start.elapsed(), std::time::Duration::ZERO);
        assert_eq!(polls, 1);
        finished.result.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn attach_waits_through_unknown() {
        let start = tokio::time::Instant::now();
        let mut polls = 0;
        let statuses = [
            JobStatus::Pending,
            JobStatus::Running,
            JobStatus::Unknown,
            JobStatus::Completed,
            JobStatus::Processed,
        ];
        let finished = JobWait::attach(poll_interval(5), None)
            .until_finished(false, job_statuses(&statuses, 3600, &mut polls))
            .await
            .unwrap();
        assert_eq!(start.elapsed(), std::time::Duration::from_secs(20));
        assert_eq!(polls, 5);
        finished.result.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn attach_budget_is_twice_job_timeout() {
        let start = tokio::time::Instant::now();
        let mut polls = 0;
        let err = JobWait::attach(poll_interval(1), None)
            .until_finished(false, job_statuses(&[JobStatus::Running], 3, &mut polls))
            .await
            .unwrap_err();
        assert!(matches!(err, RunError::JobTimeout(6)), "{err:?}");
        assert_eq!(start.elapsed(), std::time::Duration::from_secs(7));
        assert_eq!(polls, 7);
    }

    #[tokio::test(start_paused = true)]
    async fn attach_budget_is_job_timeout_option() {
        let start = tokio::time::Instant::now();
        let mut polls = 0;
        let err = JobWait::attach(poll_interval(1), Some(timeout(4)))
            .until_finished(false, job_statuses(&[JobStatus::Running], 30, &mut polls))
            .await
            .unwrap_err();
        assert!(matches!(err, RunError::JobTimeout(4)), "{err:?}");
        assert_eq!(start.elapsed(), std::time::Duration::from_secs(5));
        assert_eq!(polls, 5);
    }

    #[tokio::test(start_paused = true)]
    async fn submitted_waits_an_interval_before_the_first_poll() {
        let start = tokio::time::Instant::now();
        let mut polls = 0;
        let finished = JobWait::submitted(poll_interval(5), Some(timeout(30)))
            .until_finished(false, job_statuses(&[JobStatus::Processed], 30, &mut polls))
            .await
            .unwrap();
        assert_eq!(start.elapsed(), std::time::Duration::from_secs(5));
        assert_eq!(polls, 1);
        finished.result.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn submitted_budget_is_twice_job_timeout_option() {
        let start = tokio::time::Instant::now();
        let mut polls = 0;
        // The job's own timeout plays no part: the run that submits it sets the budget.
        let err = JobWait::submitted(poll_interval(1), Some(timeout(3)))
            .until_finished(false, job_statuses(&[JobStatus::Running], 30, &mut polls))
            .await
            .unwrap_err();
        assert!(matches!(err, RunError::JobTimeout(6)), "{err:?}");
        assert_eq!(start.elapsed(), std::time::Duration::from_secs(7));
        assert_eq!(polls, 6);
    }

    #[tokio::test(start_paused = true)]
    async fn submitted_budget_defaults_to_twice_plus_default() {
        let mut polls = 0;
        let err = JobWait::submitted(poll_interval(900), None)
            .until_finished(false, job_statuses(&[JobStatus::Pending], 30, &mut polls))
            .await
            .unwrap_err();
        assert!(matches!(err, RunError::JobTimeout(7200)), "{err:?}");
        assert_eq!(polls, 8);
    }

    #[test]
    fn failed_job_carries_its_error() {
        let finished = FinishedJob::new(json_job(JobStatus::Failed, 3, Some("exit 1"))).unwrap();
        assert_eq!(finished.report, REPORT.parse::<ReportUuid>().unwrap());
        assert!(
            matches!(&finished.result, Err(RunError::JobFailed(error)) if error == "exit 1"),
            "{finished:?}"
        );
        assert_eq!(
            finished.output.and_then(|output| output.error).as_deref(),
            Some("exit 1")
        );
    }

    #[test]
    fn canceled_job_returns_job_canceled() {
        let finished = FinishedJob::new(json_job(JobStatus::Canceled, 3, None)).unwrap();
        assert!(
            matches!(&finished.result, Err(RunError::JobCanceled(error)) if error == "Job was canceled"),
            "{finished:?}"
        );
        let finished =
            FinishedJob::new(json_job(JobStatus::Canceled, 3, Some("timed out"))).unwrap();
        assert!(
            matches!(&finished.result, Err(RunError::JobCanceled(error)) if error == "timed out"),
            "{finished:?}"
        );
    }
}
