use bencher_comment::ReportComment;
use octocrab::{
    Octocrab,
    models::CommentId,
    params::checks::{CheckRunConclusion, CheckRunOutput},
};

use crate::{cli_eprintln_quietable, cli_println_quietable};

const GITHUB_ACTIONS: &str = "GITHUB_ACTIONS";
const GITHUB_EVENT_PATH: &str = "GITHUB_EVENT_PATH";
const GITHUB_EVENT_NAME: &str = "GITHUB_EVENT_NAME";
const GITHUB_SHA: &str = "GITHUB_SHA";
const GITHUB_API_URL: &str = "GITHUB_API_URL";
const GITHUB_STEP_SUMMARY: &str = "GITHUB_STEP_SUMMARY";

const PULL_REQUEST: &str = "pull_request";
const PULL_REQUEST_TARGET: &str = "pull_request_target";

const FULL_NAME: &str = "full_name";

// https://docs.github.com/en/rest/checks/runs?apiVersion=2022-11-28#create-a-check-run
#[expect(clippy::decimal_literal_representation)]
const CHECK_MAX_LENGTH: usize = 65_536;

#[expect(clippy::struct_excessive_bools)]
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
        "Failed to read GitHub Action event path ({0}): {1}\n{mount}",
        mount = docker_mount(GITHUB_EVENT_PATH)
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
    #[error("GitHub Action event repository is missing: {0}")]
    NoRepository(String),
    #[error("GitHub Action event repository full name is missing: {0}")]
    NoFullName(String),
    #[error("GitHub Action event repository full name is invalid: {0}")]
    BadFullName(String),
    #[error("GitHub Action event repository full name is not of the form `owner/repo`: ({0})")]
    InvalidFullName(String),
    #[error("Failed to parse GitHub API URL: {}", _0.to_string())]
    BaseUri(octocrab::Error),
    #[error("Failed to authenticate as GitHub Action: {}", _0.to_string())]
    Auth(octocrab::Error),
    #[error("Failed to list GitHub PR comments: {}", _0.to_string())]
    Comments(octocrab::Error),
    #[error("Failed to create GitHub PR comment: {}", _0.to_string())]
    CreateComment(octocrab::Error),
    #[error("Failed to update GitHub PR comment: {}", _0.to_string())]
    UpdateComment(octocrab::Error),
    #[error(
        "{}",
        permissions_help("pull-requests", "pull-requests-from-forks", _0)
    )]
    BadCommentPermissions(octocrab::Error),

    #[error("Failed to get GitHub SHA\n{}", docker_env(GITHUB_SHA))]
    NoSha,
    #[error("Failed to create GitHub Check: {0}")]
    CreateCheck(octocrab::Error),
    #[error("{}", permissions_help("checks", "base-branch", _0))]
    BadCheckPermissions(octocrab::Error),
}

// https://docs.github.com/en/actions/using-jobs/assigning-permissions-to-jobs#setting-the-github_token-permissions-for-a-specific-job
fn permissions_help(scope: &str, fragment: &str, err: &octocrab::Error) -> String {
    format!(
        "GitHub Actions token (`GITHUB_TOKEN`) does not have `write` permissions for `{scope}`.\nTo fix, add `write` permissions to the job: `job: {{ \"permissions\": {{ \"{scope}\": \"write\" }} }}`\nSee: https://bencher.dev/docs/how-to/github-actions/#{fragment}\nError: {err}",
    )
}

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

        if let Ok(PULL_REQUEST) = std::env::var(GITHUB_EVENT_NAME).as_deref() {
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

    pub async fn run(&self, report_comment: &ReportComment, log: bool) -> Result<(), GitHubError> {
        if !is_github_actions() {
            cli_println_quietable!(
                log,
                "Not running as a GitHub Action. Skipping CI integration.\n{}",
                docker_env(GITHUB_ACTIONS)
            );
            return Ok(());
        }

        // Creating a job summary is not considered "posting" to CI,
        // so it is done regardless of the `ci_only_thresholds` option.
        self.create_job_summary(report_comment, log);

        // Only post to CI if there are thresholds set
        if self.ci_only_thresholds && !report_comment.has_threshold() {
            cli_println_quietable!(log, "No thresholds set. Skipping CI integration.");
            return Ok(());
        }

        let (event_str, event) = github_event()?;
        let issue_number = if let Some(issue_number) = self.ci_number {
            issue_number
        } else if let Ok(event_name @ (PULL_REQUEST | PULL_REQUEST_TARGET)) =
            // The name of the event that triggered the workflow. For example, `workflow_dispatch`.
            std::env::var(GITHUB_EVENT_NAME).as_deref()
        {
            // https://docs.github.com/en/actions/using-workflows/events-that-trigger-workflows#pull_request
            // https://docs.github.com/en/actions/using-workflows/events-that-trigger-workflows#pull_request_target
            // https://docs.github.com/en/webhooks/webhook-events-and-payloads#pull_request
            event
                .get("number")
                .ok_or_else(|| GitHubError::NoPRNumber(event_str.clone(), event_name.into()))?
                .as_u64()
                .ok_or_else(|| GitHubError::BadPRNumber(event_str.clone(), event_name.into()))?
        } else {
            cli_println_quietable!(
                log,
                "Not running as a GitHub Action pull request event (`pull_request` or `pull_request_target`) and the `--ci-number` option was not set. Creating a GitHub Check instead.\n{}",
                docker_env(GITHUB_EVENT_NAME)
            );
            return self
                .create_github_check(report_comment, log, &event_str, &event)
                .await;
        };

        self.create_pull_request_comment(report_comment, log, &event_str, &event, issue_number)
            .await
    }

    fn create_job_summary(&self, report_comment: &ReportComment, log: bool) {
        let summary = report_comment.html(self.ci_only_thresholds, self.ci_id.as_deref());
        // https://docs.github.com/en/actions/writing-workflows/choosing-what-your-workflow-does/workflow-commands-for-github-actions#adding-a-job-summary
        if let Ok(file_path) = std::env::var(GITHUB_STEP_SUMMARY)
            && let Err(err) = std::fs::write(&file_path, summary)
        {
            cli_eprintln_quietable!(
                log,
                "Failed to write GitHub Actions job summary to {file_path}: {err}"
            );
        }
    }

    async fn create_github_check(
        &self,
        report_comment: &ReportComment,
        log: bool,
        event_str: &str,
        event: &serde_json::Value,
    ) -> Result<(), GitHubError> {
        let full_name = repository_full_name(event_str, event)?;
        let (owner, repo) = split_full_name(full_name)?;
        let Ok(head_sha) = std::env::var(GITHUB_SHA) else {
            return Err(GitHubError::NoSha);
        };
        let summary = report_comment.html_with_max_length(
            self.ci_only_thresholds,
            self.ci_id.as_deref(),
            CHECK_MAX_LENGTH,
        );
        let report = CheckRunOutput {
            title: String::new(),
            summary,
            text: None,
            annotations: Vec::new(),
            images: Vec::new(),
        };

        let mut builder = Octocrab::builder().user_access_token(self.token.clone());
        if let Some(url) = github_api_url(log) {
            builder = builder.base_uri(url).map_err(GitHubError::BaseUri)?;
        }
        let check = builder
            .build()
            .map_err(GitHubError::Auth)?
            .checks(owner, repo)
            .create_check_run("Bencher Report", head_sha)
            .output(report)
            .conclusion(if report_comment.has_alert() {
                CheckRunConclusion::Failure
            } else {
                CheckRunConclusion::Success
            })
            .send()
            .await;
        if let Err(e) = check {
            Err(
                // https://github.blog/changelog/2023-02-02-github-actions-updating-the-default-github_token-permissions-to-read-only/
                if is_permissions_error(&e) {
                    GitHubError::BadCheckPermissions(e)
                } else {
                    GitHubError::CreateCheck(e)
                },
            )
        } else {
            Ok(())
        }
    }

    pub async fn create_pull_request_comment(
        &self,
        report_comment: &ReportComment,
        log: bool,
        event_str: &str,
        event: &serde_json::Value,
        issue_number: u64,
    ) -> Result<(), GitHubError> {
        let full_name = repository_full_name(event_str, event)?;
        let (owner, repo) = split_full_name(full_name)?;

        let mut builder = Octocrab::builder().user_access_token(self.token.clone());
        if let Some(url) = github_api_url(log) {
            builder = builder.base_uri(url).map_err(GitHubError::BaseUri)?;
        }
        let github_client = builder.build().map_err(GitHubError::Auth)?;

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
            Err(
                // https://github.blog/changelog/2023-02-02-github-actions-updating-the-default-github_token-permissions-to-read-only/
                if is_permissions_error(&e) {
                    GitHubError::BadCommentPermissions(e)
                } else if comment_id.is_some() {
                    GitHubError::UpdateComment(e)
                } else {
                    GitHubError::CreateComment(e)
                },
            )
        } else {
            Ok(())
        }
    }
}

// https://docs.github.com/en/actions/learn-github-actions/variables#default-environment-variables
// Always set to `true` when GitHub Actions is running the workflow. You can use this variable to differentiate when tests are being run locally or by GitHub Actions.
fn is_github_actions() -> bool {
    std::env::var(GITHUB_ACTIONS).as_deref() == Ok("true")
}

fn github_event() -> Result<(String, serde_json::Value), GitHubError> {
    // The path to the file on the runner that contains the full event webhook payload. For example, /github/workflow/event.json.
    let Ok(github_event_path) = std::env::var(GITHUB_EVENT_PATH) else {
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
            if let Some(body) = comment.body
                && body.ends_with(bencher_tag)
            {
                return Ok(Some(comment.id));
            }
        }

        if comments_len < usize::from(PER_PAGE) {
            return Ok(None);
        }

        page += 1;
    }
}

fn is_permissions_error(err: &octocrab::Error) -> bool {
    err.to_string()
        .contains("Resource not accessible by integration")
}

fn github_api_url(log: bool) -> Option<String> {
    if let Ok(url) = std::env::var(GITHUB_API_URL) {
        Some(url)
    } else {
        cli_eprintln_quietable!(
            log,
            "Failed to get GitHub API URL, defaulting to `https://api.github.com`\n{}",
            docker_env(GITHUB_API_URL)
        );
        None
    }
}
