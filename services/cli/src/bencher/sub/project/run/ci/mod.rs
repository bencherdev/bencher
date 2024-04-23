use bencher_comment::ReportComment;

use crate::parser::project::run::CliRunCi;

mod github_actions;

use github_actions::{GitHubActions, GitHubError};

#[derive(Debug)]
pub enum Ci {
    GitHubActions(GitHubActions),
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
            ci_deprecated: _,
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

    pub async fn run(&self, report_comment: &ReportComment, log: bool) -> Result<(), CiError> {
        match self {
            Self::GitHubActions(github_actions) => github_actions
                .run(report_comment, log)
                .await
                .map_err(Into::into),
        }
    }
}
