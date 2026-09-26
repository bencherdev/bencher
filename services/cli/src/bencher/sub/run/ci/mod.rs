use bencher_comment::ReportComment;
#[cfg(feature = "plus")]
use bencher_json::JsonNewCallback;
use bencher_json::ResourceName;

use crate::parser::run::CliRunCi;

#[cfg(feature = "plus")]
use super::job::CallbackNotice;

mod github_actions;

use github_actions::{CheckRunHandle, GitHubActions, GitHubError};

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
}

impl TryFrom<CliRunCi> for Option<Ci> {
    type Error = CiError;

    fn try_from(ci: CliRunCi) -> Result<Self, Self::Error> {
        let CliRunCi {
            github_actions,
            ci_only_thresholds,
            ci_only_on_alert,
            ci_public_links,
            ci_id,
            ci_number,
            ci_i_am_vulnerable_to_pwn_requests,
            #[cfg(feature = "plus")]
            ci_callback_token,
        } = ci;
        Ok(github_actions.map(|token| {
            Ci::GitHubActions(GitHubActions {
                token,
                ci_only_thresholds,
                ci_only_on_alert,
                ci_public_links,
                ci_id,
                ci_number,
                ci_i_am_vulnerable_to_pwn_requests,
                #[cfg(feature = "plus")]
                callback_token: ci_callback_token,
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

    /// Whether the CI check name needs the Project name resolved before the run.
    pub fn needs_project_name(&self) -> bool {
        match self {
            Self::GitHubActions(github_actions) => github_actions.needs_project_name(),
        }
    }

    /// Best-effort: start an in-progress check before the benchmark runs.
    pub async fn start(&self, project_name: Option<&ResourceName>, log: bool) -> Option<CiCheck> {
        match self {
            Self::GitHubActions(github_actions) => github_actions
                .start_check(project_name, log)
                .await
                .map(CiCheck::GitHubActions),
        }
    }

    pub async fn run(
        &self,
        check: Option<CiCheck>,
        report_comment: &ReportComment,
        #[cfg(feature = "plus")] failure: Option<JobFailure>,
        log: bool,
    ) -> Result<(), CiError> {
        match self {
            Self::GitHubActions(github_actions) => {
                let check = check.map(|CiCheck::GitHubActions(handle)| handle);
                github_actions
                    .run(
                        check,
                        report_comment,
                        #[cfg(feature = "plus")]
                        failure,
                        log,
                    )
                    .await
                    .map_err(Into::into)
            },
        }
    }

    /// The callback a detached run sends, so that its attach can complete the check.
    #[cfg(feature = "plus")]
    pub fn dispatch_callback(
        &self,
        check: Option<&CiCheck>,
        log: bool,
    ) -> Result<Option<JsonNewCallback>, CiError> {
        match self {
            Self::GitHubActions(github_actions) => github_actions
                .dispatch_callback(check.map(|CiCheck::GitHubActions(handle)| handle), log)
                .map_err(Into::into),
        }
    }

    /// The attach continues the detached run whose callback started it.
    #[cfg(feature = "plus")]
    pub fn read_dispatch(&mut self, log: bool) {
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

    /// Best-effort: complete a detached run's check whose callback will never fire.
    #[cfg(feature = "plus")]
    pub async fn complete_unfired(
        &self,
        check: CiCheck,
        notice: CallbackNotice,
        project_name: &ResourceName,
        log: bool,
    ) {
        match (self, check) {
            (Self::GitHubActions(github_actions), CiCheck::GitHubActions(handle)) => {
                github_actions
                    .complete_unfired_check(handle, notice, project_name, log)
                    .await;
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
