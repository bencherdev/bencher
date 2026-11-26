use bencher_comment::{GitHubApi, ReportComment};
#[cfg(feature = "plus")]
use bencher_json::runner::JsonNewJobGitHub;
use bencher_json::{JsonProject, ProjectSlug, ProjectUuid, ReportUuid};
use url::Url;

use crate::parser::run::CliRunCi;

mod github_actions;

pub(crate) use github_actions::github_api_url;
pub use github_actions::run_attempt_key;
use github_actions::{CheckRunHandle, GitHubActions, GitHubError, is_github_com_api};

#[derive(Debug)]
pub enum Ci {
    GitHubActions(GitHubActions),
}

/// A CI check started before the benchmark runs,
/// to be completed once the results are ready.
#[derive(Debug)]
pub enum CiCheck {
    GitHubActions(CheckRunHandle),
}

/// How a remote job ended without results.
#[cfg(feature = "plus")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobFailure {
    Failed,
    Canceled,
}

#[derive(thiserror::Error, Debug)]
pub enum CiError {
    #[error("{0}")]
    GitHub(#[from] GitHubError),
    #[error(
        "`--ci-callback-token` is removed: with `--detach`, Bencher posts the GitHub Check and pull request comment to the project's linked GitHub repository itself. Remove the flag."
    )]
    CallbackTokenRemoved,
}

impl TryFrom<CliRunCi> for Option<Ci> {
    type Error = CiError;

    fn try_from(ci: CliRunCi) -> Result<Self, Self::Error> {
        let CliRunCi {
            github_actions,
            ci_only_thresholds,
            ci_only_on_alert,
            ci_id,
            ci_number,
            ci_i_am_vulnerable_to_pwn_requests,
            ci_callback_token,
        } = ci;
        if ci_callback_token.is_some() {
            return Err(CiError::CallbackTokenRemoved);
        }
        Ok(github_actions.map(|token| {
            Ci::GitHubActions(GitHubActions {
                token,
                ci_only_thresholds,
                ci_only_on_alert,
                ci_id,
                ci_number,
                ci_i_am_vulnerable_to_pwn_requests,
                #[cfg(feature = "plus")]
                dispatch: None,
            })
        }))
    }
}

impl Ci {
    pub fn safety_check(&self, log: bool) -> Result<(), CiError> {
        match self {
            Self::GitHubActions(github_actions) => {
                github_actions.safety_check(log).map_err(Into::into)
            },
        }
    }

    pub fn github_api(&self, github_api_url: Option<&str>) -> GitHubApi {
        match self {
            Self::GitHubActions(_) => {
                if is_github_com_api(github_api_url) {
                    GitHubApi::GitHubCom
                } else {
                    GitHubApi::Other
                }
            },
        }
    }

    /// Whether the CI check needs the Project resolved before the run.
    pub fn needs_project(&self) -> bool {
        match self {
            Self::GitHubActions(_) => GitHubActions::needs_project(),
        }
    }

    /// Best-effort: start an in-progress check before the benchmark runs.
    pub async fn start(
        &self,
        project: Option<&JsonProject>,
        sub_adapter: &bencher_comment::SubAdapter,
        log: bool,
    ) -> Option<CiCheck> {
        match self {
            Self::GitHubActions(github_actions) => github_actions
                .start_check(project, sub_adapter, log)
                .await
                .map(CiCheck::GitHubActions),
        }
    }

    pub async fn run<R: SectionReports>(
        &self,
        check: Option<CiCheck>,
        report: &PostedReport<'_>,
        reports: &R,
        #[cfg(feature = "plus")] failure: Option<JobFailure>,
        log: bool,
    ) -> Result<(), CiError> {
        match self {
            Self::GitHubActions(github_actions) => {
                let check = check.map(|CiCheck::GitHubActions(handle)| handle);
                github_actions
                    .run(
                        check,
                        report,
                        reports,
                        #[cfg(feature = "plus")]
                        failure,
                        log,
                    )
                    .await
                    .map_err(Into::into)
            },
        }
    }

    /// The pull request context a detached run sends, so that the server can post for it.
    #[cfg(feature = "plus")]
    pub fn job_github(&self, log: bool) -> Result<Option<JsonNewJobGitHub>, CiError> {
        match self {
            Self::GitHubActions(github_actions) => {
                github_actions.job_github(log).map_err(Into::into)
            },
        }
    }

    /// The attach continues the detached run whose callback started it, and returns that run's
    /// comment tag flags.
    #[cfg(feature = "plus")]
    pub fn read_dispatch(&mut self, log: bool) -> Option<bencher_comment::SubAdapter> {
        match self {
            Self::GitHubActions(github_actions) => github_actions.read_dispatch(log),
        }
    }

    /// The check a detached run started, for its attach to complete.
    #[cfg(feature = "plus")]
    pub fn adopted_check(&self) -> Option<CiCheck> {
        match self {
            Self::GitHubActions(github_actions) => {
                github_actions.adopted_check().map(CiCheck::GitHubActions)
            },
        }
    }

    /// Best-effort: complete a still in-progress check as failed
    /// when the run errors before the results are posted.
    pub async fn fail(&self, check: &CiCheck, log: bool) {
        match (self, check) {
            (Self::GitHubActions(github_actions), CiCheck::GitHubActions(handle)) => {
                github_actions.fail_check(handle, log).await;
            },
        }
    }

    pub fn source(&self) -> String {
        match self {
            Self::GitHubActions(_) => "github".to_owned(),
        }
    }
}

/// A job's report, with what its section of the pull request comment needs of it.
pub struct PostedReport<'a> {
    pub comment: &'a ReportComment,
    pub project: ProjectUuid,
    pub report: ReportUuid,
    pub sub_adapter: bencher_comment::SubAdapter,
    pub console_url: Url,
}

/// The reports of the other jobs' sections of the pull request comment.
pub trait SectionReports {
    /// A single attempt, `None` when the report cannot be fetched.
    fn section_report(
        &self,
        console_url: &Url,
        project: &ProjectSlug,
        report: ReportUuid,
        sub_adapter: bencher_comment::SubAdapter,
    ) -> impl Future<Output = Option<ReportComment>>;
}
