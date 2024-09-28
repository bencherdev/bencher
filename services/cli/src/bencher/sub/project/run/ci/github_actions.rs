use bencher_comment::ReportComment;
use octocrab::{
    models::{checks::CheckRun, CommentId},
    params::checks::{CheckRunConclusion, CheckRunOutput},
    Octocrab,
};

use crate::{cli_println, cli_println_quietable};

const GITHUB_ACTIONS: &str = "GITHUB_ACTIONS";
const GITHUB_EVENT_PATH: &str = "GITHUB_EVENT_PATH";
const GITHUB_EVENT_NAME: &str = "GITHUB_EVENT_NAME";

const PULL_REQUEST: &str = "pull_request";
const PULL_REQUEST_TARGET: &str = "pull_request_target";
const WORKFLOW_RUN: &str = "workflow_run";

const FULL_NAME: &str = "full_name";

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug)]
pub struct GitHubActions {
    pub token: String,
    pub ci_only_thresholds: bool,
    pub ci_only_on_alert: bool,
    pub ci_public_links: bool,
    pub ci_id: Option<String>,
    pub ci_number: Option<u64>,
    pub ci_i_am_vulnerable_to_pwn_requests: bool,
}

#[derive(thiserror::Error, Debug)]
pub enum GitHubError {
    #[error(
        "Failed to get GitHub Action event path\n{}",
        docker_env(GITHUB_EVENT_PATH)
    )]
    NoEventPath,
    #[error(
        "Failed to read GitHub Action event path ({0}): {1}\n{}",
        docker_mount(GITHUB_EVENT_PATH)
    )]
    BadEventPath(String, std::io::Error),
    #[error("Failed to parse GitHub Action event ({0}): {1}\n")]
    BadEvent(String, serde_json::Error),

    #[error("GitHub Action event pull request is missing: {0}")]
    NoPullRequest(String),
    #[error("GitHub Action event pull request head is missing: {0}")]
    NoHead(String),
    #[error("GitHub Action event pull request head repo is missing: {0}")]
    NoHeadRepo(String),
    #[error("GitHub Action event pull request head repo full name is missing: {0}")]
    NoHeadFullName(String),
    #[error("GitHub Action event pull request head repo full name  is invalid: {0}")]
    BadHeadFullName(String),
    #[error("{}", pwn_requests(head, base))]
    PwnRequest { head: String, base: String },

    #[error("GitHub Action event ({1}) PR number is missing: {0}")]
    NoPRNumber(String, String),
    #[error("GitHub Action event ({1}) PR number is invalid: {0}")]
    BadPRNumber(String, String),
    #[error(
        "GitHub Action for workflow run must explicitly set PR number (ex: `--ci-number 123`)"
    )]
    NoWorkflowRunPRNumber,
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
    #[error("GitHub Actions token (`GITHUB_TOKEN`) does not have `write` permissions for `pull-requests`.\n{help}\nError: {0}", help = PERMISSIONS_HELP)]
    BadPermissions(octocrab::Error),
    #[error("Failed to create GitHub check: {0}")]
    FailedCreatingCheck(octocrab::Error),
    #[error("No GITHUB_SHA is set")]
    NoSHA,
}

// https://docs.github.com/en/actions/using-jobs/assigning-permissions-to-jobs#setting-the-github_token-permissions-for-a-specific-job
const PERMISSIONS_HELP: &str = "To fix, add `write` permissions to the job: `job: {{ \"permissions\": {{ \"pull-requests\": \"write\" }} }}`\nSee: https://bencher.dev/docs/how-to/github-actions/#pull-requests";

fn docker_env(env_var: &str) -> String {
    format!(
        "If you are running in a Docker container, then you need to pass in the `{env_var}` environment variable. See https://bencher.dev/docs/explanation/bencher-run/#--github-actions",
    )
}

fn docker_mount(env_var: &str) -> String {
    format!(
        "If you are running in a Docker container, then you need mount the path specified by `{env_var}`. See https://bencher.dev/docs/explanation/bencher-run/#--github-actions",
    )
}

fn pwn_requests(base: &str, head: &str) -> String {
    format!(
        "WARNING! Unsafe use of GitHub Actions `pull_request` event!\nThis is a pull request from a forked repository owned by {head} to you ({base}). This is a major security risk!\nFor safer options, see: https://bencher.dev/docs/how-to/github-actions/#pull-requests-from-forks\nFor more information on pwn requests, see: https://securitylab.github.com/research/github-actions-preventing-pwn-requests/",
    )
}

impl GitHubActions {
    pub fn safety_check(&self, log: bool) -> Result<(), GitHubError> {
        if !is_github_actions() {
            return Ok(());
        }

        let (event_str, event) = github_event()?;

        if let Some(PULL_REQUEST) = std::env::var(GITHUB_EVENT_NAME).ok().as_deref() {
            let head = event
                .get("pull_request")
                .ok_or_else(|| GitHubError::NoPullRequest(event_str.clone()))?
                .get("head")
                .ok_or_else(|| GitHubError::NoHead(event_str.clone()))?
                .get("repo")
                .ok_or_else(|| GitHubError::NoHeadRepo(event_str.clone()))?
                .get(FULL_NAME)
                .ok_or_else(|| GitHubError::NoHeadFullName(event_str.clone()))?
                .as_str()
                .ok_or_else(|| GitHubError::BadHeadFullName(event_str.clone()))?;
            let (head_owner, _) = split_full_name(head)?;

            let base = repository_full_name(&event_str, &event)?;
            let (base_owner, _) = split_full_name(base)?;

            if head_owner != base_owner {
                if self.ci_i_am_vulnerable_to_pwn_requests {
                    cli_println_quietable!(log, "{}", pwn_requests(head_owner, base_owner));
                } else {
                    return Err(GitHubError::PwnRequest {
                        head: head.to_owned(),
                        base: base.to_owned(),
                    });
                }
            }
        }

        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    pub async fn run(&self, report_comment: &ReportComment, log: bool) -> Result<(), GitHubError> {
        // Only post to CI if there are thresholds set
        if self.ci_only_thresholds && !report_comment.has_threshold() {
            cli_println_quietable!(log, "No thresholds set. Skipping CI integration.");
            return Ok(());
        }

        if !is_github_actions() {
            cli_println_quietable!(
                log,
                "Not running as a GitHub Action. Skipping CI integration.\n{}",
                docker_env(GITHUB_ACTIONS)
            );
            return Ok(());
        }

        let (event_str, event) = github_event()?;

        // The name of the event that triggered the workflow. For example, `workflow_dispatch`.
        let issue_number = match std::env::var(GITHUB_EVENT_NAME).ok().as_deref() {
            // https://docs.github.com/en/actions/using-workflows/events-that-trigger-workflows#pull_request
            // https://docs.github.com/en/actions/using-workflows/events-that-trigger-workflows#pull_request_target
            Some(event_name @ (PULL_REQUEST | PULL_REQUEST_TARGET)) => {
                // https://docs.github.com/en/webhooks/webhook-events-and-payloads#pull_request
                if let Some(issue_number) = self.ci_number {
                    issue_number
                } else {
                    event
                        .get("number")
                        .ok_or_else(|| {
                            GitHubError::NoPRNumber(event_str.clone(), event_name.into())
                        })?
                        .as_u64()
                        .ok_or_else(|| {
                            GitHubError::BadPRNumber(event_str.clone(), event_name.into())
                        })?
                }
            },
            // https://docs.github.com/en/actions/using-workflows/events-that-trigger-workflows#workflow_run
            Some(WORKFLOW_RUN) => {
                // https://docs.github.com/en/webhooks/webhook-events-and-payloads#workflow_run
                self.ci_number.ok_or(GitHubError::NoWorkflowRunPRNumber)?
            },
            _ => {
                cli_println!(
                    "Not running as usual GitHub Action event (`pull_request`, `pull_request_target`, or `workflow_run`). Making GitHub Checks instead.\n{}",
                    docker_env(GITHUB_EVENT_NAME)
                );
                self.create_github_check(report_comment, &event_str, &event)
                    .await?;
                return Ok(());
            },
        };

        let full_name = repository_full_name(&event_str, &event)?;
        let (owner, repo) = split_full_name(full_name)?;

        let github_client = Octocrab::builder()
            .user_access_token(self.token.clone())
            .build()
            .map_err(GitHubError::Auth)?;

        // Get the comment ID if it exists
        let comment_id = get_comment(
            &github_client,
            owner,
            repo,
            issue_number,
            &report_comment.bencher_tag(self.ci_id.as_deref()),
        )
        .await?;

        // Update or create the comment
        let issue_handler = github_client.issues(owner, repo);
        let body = report_comment.html(self.ci_only_thresholds, self.ci_id.as_deref());
        // Always update the comment if it exists
        let comment = if let Some(comment_id) = comment_id {
            issue_handler.update_comment(comment_id, body).await
        } else {
            if self.ci_only_on_alert && !report_comment.has_alert() {
                cli_println_quietable!(log, "No alerts found. Skipping CI integration.");
                return Ok(());
            }
            issue_handler.create_comment(issue_number, body).await
        };
        if let Err(e) = comment {
            return Err(
                // https://github.blog/changelog/2023-02-02-github-actions-updating-the-default-github_token-permissions-to-read-only/
                if e.to_string()
                    .contains("Resource not accessible by integration")
                {
                    GitHubError::BadPermissions(e)
                } else if comment_id.is_some() {
                    GitHubError::UpdateComment(e)
                } else {
                    GitHubError::CreateComment(e)
                },
            );
        }

        Ok(())
    }

    async fn create_github_check(
        &self,
        report_comment: &ReportComment,
        event_str: &str,
        event: &serde_json::Value,
    ) -> Result<CheckRun, GitHubError> {
        let full_name = repository_full_name(event_str, event)?;
        let (owner, repo) = split_full_name(full_name)?;

        let report = CheckRunOutput {
            title: "Bencher Report".to_owned(),
            summary: report_comment.html(self.ci_only_thresholds, self.ci_id.as_deref()),
            text: None,
            annotations: Vec::new(),
            images: Vec::new(),
        };
        Octocrab::builder()
            .user_access_token(self.token.clone())
            .build()
            .map_err(GitHubError::Auth)?
            .checks(owner, repo)
            .create_check_run(
                "bencher",
                std::env::var("GITHUB_SHA").map_err(|_err| GitHubError::NoSHA)?,
            )
            .output(report)
            .conclusion(if report_comment.has_alert() {
                // TODO: action required
                CheckRunConclusion::Failure
            } else {
                CheckRunConclusion::Success
            })
            .send()
            .await
            .map_err(GitHubError::FailedCreatingCheck)
    }
}

// https://docs.github.com/en/actions/learn-github-actions/variables#default-environment-variables
// Always set to `true` when GitHub Actions is running the workflow. You can use this variable to differentiate when tests are being run locally or by GitHub Actions.
fn is_github_actions() -> bool {
    std::env::var(GITHUB_ACTIONS).ok().as_deref() == Some("true")
}

fn github_event() -> Result<(String, serde_json::Value), GitHubError> {
    // The path to the file on the runner that contains the full event webhook payload. For example, /github/workflow/event.json.
    let Some(github_event_path) = std::env::var(GITHUB_EVENT_PATH).ok() else {
        return Err(GitHubError::NoEventPath);
    };
    let event_str = std::fs::read_to_string(&github_event_path)
        .map_err(|e| GitHubError::BadEventPath(github_event_path, e))?;
    // The event JSON does not match the GitHub API event JSON schema used by Octocrab
    // Therefore we use serde_json::Value to parse the event
    let event = serde_json::from_str(&event_str)
        .map_err(|e| GitHubError::BadEvent(event_str.clone(), e))?;
    Ok((event_str, event))
}

// Use the full name instead of getting the owner and repo names separately
// because the owner name values in the API are nullable
// https://docs.github.com/en/rest/repos/repos#get-a-repository
// The owner and repository name. For example, octocat/Hello-World.
fn repository_full_name<'e>(
    event_str: &str,
    event: &'e serde_json::Value,
) -> Result<&'e str, GitHubError> {
    event
        .get("repository")
        .ok_or_else(|| GitHubError::NoRepository(event_str.to_owned()))?
        .get(FULL_NAME)
        .ok_or_else(|| GitHubError::NoFullName(event_str.to_owned()))?
        .as_str()
        .ok_or_else(|| GitHubError::BadFullName(event_str.to_owned()))
}

fn split_full_name(full_name: &str) -> Result<(&str, &str), GitHubError> {
    full_name
        .split_once('/')
        .ok_or_else(|| GitHubError::InvalidFullName(full_name.to_owned()))
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
        }

        page += 1;
    }
}
