use bencher_json::{
    JsonJob, Url,
    runner::{JobGitHubFailure, JobGitHubState, JsonJobGitHub, JsonNewJobGitHub},
};

use super::RunError;

/// The API client's type, filled from the validated pull request context.
pub fn client_github(
    github: &JsonNewJobGitHub,
) -> Result<bencher_client::types::JsonNewJobGitHub, RunError> {
    serde_json::to_value(github)
        .and_then(serde_json::from_value)
        .map_err(RunError::ClientGitHub)
}

/// What the read after submit prints of the server's post of the check and comment.
pub fn post_notice(json_job: &JsonJob) -> Option<String> {
    let JsonJobGitHub {
        state,
        status,
        failure,
        approval_url,
    } = json_job.github.as_ref()?;
    Some(match state {
        JobGitHubState::Pending => {
            "Bencher posts the results to GitHub when the job finishes".to_owned()
        },
        JobGitHubState::Posted => "Bencher posted the results to GitHub".to_owned(),
        JobGitHubState::Failed => format!(
            "Warning: Bencher failed to post the results to GitHub: {}",
            failure_reason(*failure, *status, approval_url.as_ref())
        ),
    })
}

fn failure_reason(
    failure: Option<JobGitHubFailure>,
    status: Option<u16>,
    approval_url: Option<&Url>,
) -> String {
    let reason = match failure {
        Some(JobGitHubFailure::Approval) => {
            "the GitHub App's installation has not approved Checks write and Pull requests write"
        },
        Some(JobGitHubFailure::Unlinked) => {
            "the project is no longer linked to this repository, so link it again in the project's Settings"
        },
        Some(JobGitHubFailure::Refused) => "GitHub refused a request",
        Some(JobGitHubFailure::Exhausted) => "every attempt failed",
        None => "the post failed",
    };
    let status = status
        .map(|status| format!(" (HTTP {status})"))
        .unwrap_or_default();
    let approve = approval_url
        .map(|url| format!(". Approve them at {url}"))
        .unwrap_or_default();
    format!("{reason}{status}{approve}")
}
