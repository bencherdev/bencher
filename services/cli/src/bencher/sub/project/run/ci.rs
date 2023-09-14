use bencher_json::NonEmpty;
use octocrab::{models::CommentId, Octocrab};

use crate::parser::project::run::CliRunCi;

use super::urls::ReportUrls;
use crate::cli_println;

#[derive(Debug)]
pub enum Ci {
    GitHubActions(GitHubActions),
}

#[derive(thiserror::Error, Debug)]
pub enum CiError {
    #[error("{0}")]
    GitHub(#[from] GitHubError),
}

#[derive(thiserror::Error, Debug)]
pub enum GitHubError {
    #[error("Failed to get GitHub Action event path")]
    NoEventPath,
    #[error("Failed to read GitHub Action event path ({0}): {1}")]
    BadEventPath(String, std::io::Error),
    #[error("Failed to parse GitHub Action event ({0}): {1}")]
    BadEvent(String, serde_json::Error),
    #[error("GitHub Action event ({1}) PR number is missing: {0}")]
    NoPRNumber(String, String),
    #[error("GitHub Action event ({1}) PR number is invalid: {0}")]
    BadPRNumber(String, String),
    #[error("GitHub Action event workflow run is missing: {0}")]
    NoWorkFlowRun(String),
    #[error("GitHub Action event workflow run pull requests is missing: {0}")]
    NoPullRequests(String),
    #[error("GitHub Action event workflow run pull requests is empty: {0}")]
    EmptyPullRequests(String),
    #[error("GitHub Action event repository is missing: {0}")]
    NoRepository(String),
    #[error("GitHub Action event repository full name is missing: {0}")]
    NoFullName(String),
    #[error("GitHub Action event repository full name is invalid: {0}")]
    BadFullName(String),
    #[error("GitHub Action event repository full name is not of the form `owner/repo`: ({0})")]
    InvalidFullName(String),
    #[error("Failed to authenticate as GitHub Action: {0}")]
    Auth(octocrab::Error),
    #[error("Failed to list GitHub PR comments: {0}")]
    Comments(octocrab::Error),
    #[error("Failed to create GitHub PR comment: {0}")]
    CreateComment(octocrab::Error),
    #[error("Failed to update GitHub PR comment: {0}")]
    UpdateComment(octocrab::Error),
}

impl TryFrom<CliRunCi> for Option<Ci> {
    type Error = CiError;

    fn try_from(ci: CliRunCi) -> Result<Self, Self::Error> {
        let CliRunCi {
            ci_only_thresholds,
            ci_only_on_alert,
            ci_id,
            github_actions,
        } = ci;
        Ok(github_actions.map(|github_actions| {
            Ci::GitHubActions(GitHubActions::new(
                ci_only_thresholds,
                ci_only_on_alert,
                ci_id,
                github_actions,
            ))
        }))
    }
}

impl Ci {
    pub async fn run(&self, report_urls: &ReportUrls) -> Result<(), CiError> {
        match self {
            Self::GitHubActions(github_actions) => {
                github_actions.run(report_urls).await.map_err(Into::into)
            },
        }
    }
}

#[derive(Debug)]
pub struct GitHubActions {
    ci_only_thresholds: bool,
    ci_only_on_alert: bool,
    ci_id: Option<NonEmpty>,
    token: String,
}

impl GitHubActions {
    fn new(
        ci_only_thresholds: bool,
        ci_only_on_alert: bool,
        ci_id: Option<NonEmpty>,
        token: String,
    ) -> Self {
        Self {
            ci_only_thresholds,
            ci_only_on_alert,
            ci_id,
            token,
        }
    }

    pub async fn run(&self, report_urls: &ReportUrls) -> Result<(), GitHubError> {
        // Only post to CI if there are thresholds set
        if self.ci_only_thresholds && !report_urls.has_threshold() {
            cli_println!("No thresholds set. Skipping CI integration.");
            return Ok(());
        }

        // https://docs.github.com/en/actions/learn-github-actions/variables#default-environment-variables
        // Always set to `true` when GitHub Actions is running the workflow. You can use this variable to differentiate when tests are being run locally or by GitHub Actions.
        match std::env::var("GITHUB_ACTIONS").ok() {
            Some(github_actions) if github_actions == "true" => {},
            _ => {
                cli_println!("Not running as a GitHub Action. Skipping CI integration.");
                return Ok(());
            },
        }

        // The path to the file on the runner that contains the full event webhook payload. For example, /github/workflow/event.json.
        let github_event_path = match std::env::var("GITHUB_EVENT_PATH").ok() {
            Some(github_event_path) => github_event_path,
            _ => return Err(GitHubError::NoEventPath),
        };
        let event_str = std::fs::read_to_string(&github_event_path)
            .map_err(|e| GitHubError::BadEventPath(github_event_path, e))?;
        // The event JSON does not match the GitHub API event JSON schema used by Octocrab
        // Therefore we use serde_json::Value to parse the event
        let event: serde_json::Value = serde_json::from_str(&event_str)
            .map_err(|e| GitHubError::BadEvent(event_str.clone(), e))?;

        const NUMBER_KEY: &str = "number";
        // The name of the event that triggered the workflow. For example, `workflow_dispatch`.
        let issue_number = match std::env::var("GITHUB_EVENT_NAME").ok().as_deref() {
            // https://docs.github.com/en/actions/using-workflows/events-that-trigger-workflows#pull_request
            // https://docs.github.com/en/actions/using-workflows/events-that-trigger-workflows#pull_request_target
            Some(event_name @ ("pull_request" | "pull_request_target")) => {
                // https://docs.github.com/en/webhooks/webhook-events-and-payloads#pull_request
                event
                    .get(NUMBER_KEY)
                    .ok_or_else(|| GitHubError::NoPRNumber(event_str.clone(), event_name.into()))?
                    .as_u64()
                    .ok_or_else(|| GitHubError::BadPRNumber(event_str.clone(), event_name.into()))?
            },
            // https://docs.github.com/en/actions/using-workflows/events-that-trigger-workflows#workflow_run
            Some(event_name @ "workflow_run") => {
                // https://docs.github.com/en/webhooks/webhook-events-and-payloads#workflow_run
                event
                    .get(event_name)
                    .ok_or_else(|| GitHubError::NoWorkFlowRun(event_str.clone()))?
                    .get("pull_requests")
                    .ok_or_else(|| GitHubError::NoPullRequests(event_str.clone()))?
                    .get(0)
                    .ok_or_else(|| GitHubError::EmptyPullRequests(event_str.clone()))?
                    .get(NUMBER_KEY)
                    .ok_or_else(|| GitHubError::NoPRNumber(event_str.clone(), event_name.into()))?
                    .as_u64()
                    .ok_or_else(|| GitHubError::BadPRNumber(event_str.clone(), event_name.into()))?
            },
            _ => {
                cli_println!(
                    "Not running as an expected GitHub Action event (`pull_request`, `pull_request_target`, or `workflow_run`). Skipping CI integration."
                );
                return Ok(());
            },
        };

        // Use the full name instead of getting the owner and repo names separately
        // because the owner name values in the API are nullable
        // https://docs.github.com/en/rest/repos/repos#get-a-repository
        let full_name = event
            .get("repository")
            .ok_or_else(|| GitHubError::NoRepository(event_str.clone()))?
            .get("full_name")
            .ok_or_else(|| GitHubError::NoFullName(event_str.clone()))?
            .as_str()
            .ok_or_else(|| GitHubError::BadFullName(event_str.clone()))?;
        // The owner and repository name. For example, octocat/Hello-World.
        let (owner, repo) = if let Some((owner, repo)) = full_name.split_once('/') {
            (owner.to_owned(), repo.to_owned())
        } else {
            return Err(GitHubError::InvalidFullName(full_name.into()));
        };

        let github_client = Octocrab::builder()
            .user_access_token(self.token.clone())
            .build()
            .map_err(GitHubError::Auth)?;

        // Get the comment ID if it exists
        let comment_id = get_comment(
            &github_client,
            &owner,
            &repo,
            issue_number,
            &report_urls.bencher_tag(self.ci_id.clone()),
        )
        .await?;

        // Update or create the comment
        let issue_handler = github_client.issues(owner, repo);
        let body = report_urls.html(self.ci_only_thresholds, self.ci_id.clone());
        let _comment = if let Some(comment_id) = comment_id {
            issue_handler
                .update_comment(comment_id, body)
                .await
                .map_err(GitHubError::UpdateComment)?
        } else {
            if self.ci_only_on_alert && !report_urls.has_alert() {
                cli_println!("No alerts found. Skipping CI integration.");
                return Ok(());
            }
            issue_handler
                .create_comment(issue_number, body)
                .await
                .map_err(GitHubError::CreateComment)?
        };

        Ok(())
    }
}

pub async fn get_comment(
    github_client: &Octocrab,
    owner: &str,
    repo: &str,
    issue_number: u64,
    bencher_tag: &str,
) -> Result<Option<CommentId>, GitHubError> {
    const PER_PAGE: u8 = 100;

    let mut page: u32 = 1;
    loop {
        let comments = github_client
            .issues(owner, repo)
            .list_comments(issue_number)
            .per_page(PER_PAGE)
            .page(page)
            .send()
            .await
            .map_err(GitHubError::Comments)?;

        let comments_len = comments.items.len();
        if comments_len == 0 {
            return Ok(None);
        }

        for comment in comments.items {
            if let Some(body) = comment.body {
                if body.ends_with(bencher_tag) {
                    return Ok(Some(comment.id));
                }
            }
        }

        if comments_len < usize::from(PER_PAGE) {
            return Ok(None);
        } else {
            page += 1;
        }
    }
}
