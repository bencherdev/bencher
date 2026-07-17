use bencher_comment::ReportComment;

use crate::parser::run::CliRunCi;

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

    /// Best-effort: start an in-progress check before the benchmark runs.
    pub async fn start(&self, log: bool) -> Option<CiCheck> {
        match self {
            Self::GitHubActions(github_actions) => github_actions
                .start_check(log)
                .await
                .map(CiCheck::GitHubActions),
        }
    }

    pub async fn run(
        &self,
        check: Option<CiCheck>,
        report_comment: &ReportComment,
        log: bool,
    ) -> Result<(), CiError> {
        match self {
            Self::GitHubActions(github_actions) => {
                let check = check.map(|CiCheck::GitHubActions(handle)| handle);
                github_actions
                    .run(check, report_comment, log)
                    .await
                    .map_err(Into::into)
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
