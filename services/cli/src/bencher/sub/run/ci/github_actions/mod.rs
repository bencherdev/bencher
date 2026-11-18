use bencher_comment::{
    IN_PROGRESS_SUMMARY, MAX_LENGTH, ReportComment, SectionCheck, branded_summary, check_run_id,
    check_run_name,
};
#[cfg(feature = "plus")]
use bencher_json::{
    CiId, GitHash, GitHubIssueNumber, GitHubRepositoryId, ValidError, runner::JsonNewJobGitHub,
};
use bencher_json::{JsonProject, Secret};
use octocrab::{
    Octocrab,
    models::{CheckRunId, CommentId},
    params::checks::{CheckRunConclusion, CheckRunOutput, CheckRunStatus},
};
#[cfg(feature = "plus")]
use serde::Deserialize as _;

use crate::{cli_eprintln_quietable, cli_println_quietable};

#[cfg(feature = "plus")]
use super::JobFailure;
use super::{PostedReport, SectionReports};

mod external_id;
mod group;
mod section;

use external_id::{ExternalId, Kind};
use group::{Author, CheckApp, GroupWrite, Outcome, OwnSection};
use section::SectionKey;

const GITHUB_ACTIONS: &str = "GITHUB_ACTIONS";
const GITHUB_EVENT_PATH: &str = "GITHUB_EVENT_PATH";
const GITHUB_EVENT_NAME: &str = "GITHUB_EVENT_NAME";
const GITHUB_SHA: &str = "GITHUB_SHA";
const GITHUB_REPOSITORY: &str = "GITHUB_REPOSITORY";
const GITHUB_RUN_ID: &str = "GITHUB_RUN_ID";
const GITHUB_RUN_ATTEMPT: &str = "GITHUB_RUN_ATTEMPT";
const ACT: &str = "ACT";
const GITHUB_API_URL: &str = "GITHUB_API_URL";
const GITHUB_STEP_SUMMARY: &str = "GITHUB_STEP_SUMMARY";

const PULL_REQUEST: &str = "pull_request";
const PULL_REQUEST_TARGET: &str = "pull_request_target";
#[cfg(feature = "plus")]
const REPOSITORY_DISPATCH: &str = "repository_dispatch";

const DEFAULT_GITHUB_API_URL: &str = "https://api.github.com";

const FULL_NAME: &str = "full_name";

const FAILED_SUMMARY: &str = "<p><code>bencher run</code> failed to complete before posting results. See the CI job logs for details.</p>";

#[derive(Debug)]
pub struct GitHubActions {
    pub token: Secret,
    pub ci_only_thresholds: bool,
    pub ci_only_on_alert: bool,
    pub ci_id: Option<String>,
    pub ci_number: Option<u64>,
    pub ci_i_am_vulnerable_to_pwn_requests: bool,
    /// What the attach took over from the `repository_dispatch` of a detached run.
    #[cfg(feature = "plus")]
    pub dispatch: Option<Dispatched>,
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
    #[error("Failed to get GitHub PR comment: {}", _0.to_string())]
    Comment(octocrab::Error),
    #[error("Failed to delete GitHub PR comment: {}", _0.to_string())]
    DeleteComment(octocrab::Error),
    #[error("Failed to get GitHub pull request: {}", _0.to_string())]
    PullRequest(octocrab::Error),
    #[error("Failed to list GitHub Checks: {}", _0.to_string())]
    CheckRuns(octocrab::Error),
    #[error(
        "{}",
        permissions_help("pull-requests", "pull-requests-from-forks", _0)
    )]
    BadCommentPermissions(octocrab::Error),

    #[error("Failed to get GitHub SHA\n{}", docker_env(GITHUB_SHA))]
    NoSha,
    #[error("Failed to create GitHub Check: {0}")]
    CreateCheck(octocrab::Error),
    #[error("Failed to start GitHub Check: {0}")]
    StartCheck(octocrab::Error),
    #[error("Failed to update GitHub Check: {0}")]
    UpdateCheck(octocrab::Error),
    #[error("{}", permissions_help("checks", "base-branch", _0))]
    BadCheckPermissions(octocrab::Error),

    #[cfg(feature = "plus")]
    #[error("GitHub head SHA is not a commit hash: {0}")]
    BadSha(ValidError),
    #[cfg(feature = "plus")]
    #[error("GitHub Action event repository ID is missing or invalid: {0}")]
    BadRepositoryId(String),
    #[cfg(feature = "plus")]
    #[error("GitHub pull request number is invalid: {0}")]
    BadNumber(ValidError),
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

    /// Whether the Check needs the Project resolved before the run:
    /// its name when there is no `--ci-id`, and its UUID for the `external_id`.
    pub fn needs_project() -> bool {
        is_github_actions()
    }

    /// Best-effort: create the GitHub Check with an `in_progress` status
    /// before the benchmark runs, so a rerun immediately clears the stale
    /// conclusion left by a previous run. Failures are logged, never fatal.
    pub async fn start_check(
        &self,
        project: Option<&JsonProject>,
        sub_adapter: &bencher_comment::SubAdapter,
        log: bool,
    ) -> Option<CheckRunHandle> {
        if !is_github_actions() {
            // `run` prints the "Not running as a GitHub Action" message later,
            // so stay quiet here to avoid printing it twice.
            return None;
        }
        match self.try_start_check(project, sub_adapter, log).await {
            Ok(handle) => Some(handle),
            Err(err) => {
                cli_eprintln_quietable!(
                    log,
                    "Failed to start GitHub Check. The check will still be created when the results are posted.\n{err}"
                );
                None
            },
        }
    }

    async fn try_start_check(
        &self,
        project: Option<&JsonProject>,
        sub_adapter: &bencher_comment::SubAdapter,
        log: bool,
    ) -> Result<CheckRunHandle, GitHubError> {
        let id = check_run_id(self.ci_id.as_deref(), project.map(|project| &project.name));
        let external_id = ExternalId {
            project: project.map(|project| project.uuid),
            report: None,
            kind: Kind::new(self.ci_id.as_deref(), sub_adapter),
        };
        let (event_str, event) = github_event()?;
        let full_name = repository_full_name(&event_str, &event)?;
        let (owner, repo) = split_full_name(full_name)?;
        let head_sha = head_sha(
            &event,
            #[cfg(feature = "plus")]
            self.dispatch.as_ref(),
        )?;
        let check = self
            .github_client(log)?
            .checks(owner, repo)
            .create_check_run(check_run_name(id), head_sha)
            .external_id(external_id.to_string())
            .status(CheckRunStatus::InProgress)
            .output(check_run_output(branded_summary(IN_PROGRESS_SUMMARY)))
            .send()
            .await
            .map_err(|e| check_error(e, GitHubError::StartCheck))?;
        Ok(CheckRunHandle {
            owner: owner.to_owned(),
            repo: repo.to_owned(),
            id: check.id,
        })
    }

    pub async fn run<R: SectionReports>(
        &self,
        check: Option<CheckRunHandle>,
        report: &PostedReport<'_>,
        reports: &R,
        #[cfg(feature = "plus")] failure: Option<JobFailure>,
        log: bool,
    ) -> Result<(), GitHubError> {
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
        self.create_job_summary(report.comment, log);

        let (event_str, event) = github_event()?;

        // Always complete the GitHub Check, best-effort.
        // The check is completed regardless of `--ci-only-thresholds` and
        // `--ci-only-on-alert` so that a `success`/`failure` conclusion is always
        // reported and the check can be used as a required status check.
        let own_check = self
            .complete_github_check(
                check,
                report,
                #[cfg(feature = "plus")]
                failure,
                log,
                &event_str,
                &event,
            )
            .await
            .inspect_err(|err| {
                cli_eprintln_quietable!(log, "Failed to complete GitHub Check\n{err}");
            })
            .ok();

        let Some(issue_number) = self.issue_number(&event_str, &event)? else {
            #[cfg(feature = "plus")]
            if self.dispatch.is_some() {
                cli_println_quietable!(
                    log,
                    "The `repository_dispatch` payload has no pull request number and `--ci-number` was not set. Skipping PR comment."
                );
                return Ok(());
            }
            cli_println_quietable!(
                log,
                "Not running as a GitHub Action pull request event (`pull_request` or `pull_request_target`) and the `--ci-number` option was not set. Skipping PR comment.\n{}",
                docker_env(GITHUB_EVENT_NAME)
            );
            return Ok(());
        };

        // Only a comment by the bot of the app that owns the check is read, so without it there is no group.
        if let Some((own_check, Some(author))) = own_check {
            let kind = Kind::new(self.ci_id.as_deref(), &report.sub_adapter);
            let full_name = repository_full_name(&event_str, &event)?;
            let (owner, repo) = split_full_name(full_name)?;
            let head_sha = head_sha(
                &event,
                #[cfg(feature = "plus")]
                self.dispatch.as_ref(),
            )?;
            if let Some(key) = SectionKey::new(kind, &own_check, Some(report.comment))
                && let Ok(head_sha) = head_sha.parse()
            {
                let group = GroupWrite {
                    client: self.github_client(log)?,
                    owner,
                    repo,
                    issue_number,
                    head_sha,
                    project: report.project,
                    project_slug: report.comment.project_slug(),
                    own: OwnSection {
                        key,
                        check: own_check,
                        report: report.comment,
                    },
                    author,
                    reports,
                    console_url: &report.console_url,
                    require_threshold: self.ci_only_thresholds,
                    only_on_alert: self.ci_only_on_alert,
                    log,
                };
                if let Outcome::Done = group.write().await? {
                    return Ok(());
                }
            }
        }

        // Today's comment, for this job alone.
        if self.ci_only_thresholds && !report.comment.has_threshold() {
            cli_println_quietable!(log, "No thresholds set. Skipping pull request comment.");
            return Ok(());
        }
        self.create_pull_request_comment(report.comment, log, &event_str, &event, issue_number)
            .await
    }

    /// `--ci-number`, else the number of the pull request the event is for.
    fn issue_number(
        &self,
        event_str: &str,
        event: &serde_json::Value,
    ) -> Result<Option<u64>, GitHubError> {
        if let Some(issue_number) = self.ci_number {
            return Ok(Some(issue_number));
        }
        // The name of the event that triggered the workflow. For example, `workflow_dispatch`.
        let event_name = std::env::var(GITHUB_EVENT_NAME).ok();
        let Some(event_name @ (PULL_REQUEST | PULL_REQUEST_TARGET)) = event_name.as_deref() else {
            return Ok(None);
        };
        // https://docs.github.com/en/actions/using-workflows/events-that-trigger-workflows#pull_request
        // https://docs.github.com/en/actions/using-workflows/events-that-trigger-workflows#pull_request_target
        // https://docs.github.com/en/webhooks/webhook-events-and-payloads#pull_request
        event
            .get("number")
            .ok_or_else(|| GitHubError::NoPRNumber(event_str.to_owned(), event_name.into()))?
            .as_u64()
            .ok_or_else(|| GitHubError::BadPRNumber(event_str.to_owned(), event_name.into()))
            .map(Some)
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

    /// The completed check, as the job's own section records it.
    async fn complete_github_check(
        &self,
        check: Option<CheckRunHandle>,
        report: &PostedReport<'_>,
        #[cfg(feature = "plus")] failure: Option<JobFailure>,
        log: bool,
        event_str: &str,
        event: &serde_json::Value,
    ) -> Result<(SectionCheck, Option<Author>), GitHubError> {
        let report_comment = report.comment;
        let summary = report_comment.html_with_max_length(
            self.ci_only_thresholds,
            self.ci_id.as_deref(),
            MAX_LENGTH,
        );
        let output = check_run_output(summary);
        let conclusion = check_conclusion(
            report_comment.has_alert(),
            #[cfg(feature = "plus")]
            failure,
        );
        let external_id = ExternalId {
            project: Some(report.project),
            report: Some(report.report),
            kind: Kind::new(self.ci_id.as_deref(), &report.sub_adapter),
        }
        .to_string();
        // The Report always names its Project, so the completed check is always
        // named for it, even when the pre-run lookup could not name the
        // in-progress check. Required status checks match by exact name, so the
        // completed name has to be the same whether or not that lookup worked.
        let name = check_run_name(check_run_id(
            self.ci_id.as_deref(),
            Some(report_comment.project_name()),
        ));

        // Providing a conclusion sets the check status to `completed`.
        let completion = Completion {
            // A no-op on an update unless the in-progress check fell back to a bare name.
            name: &name,
            output,
            conclusion,
            external_id: &external_id,
        };
        // The response names the app whose bot writes the pull request comment.
        let completed: CompletedCheck = if let Some(CheckRunHandle { owner, repo, id }) = check {
            self.github_client(log)?
                .patch(
                    format!("/repos/{owner}/{repo}/check-runs/{id}"),
                    Some(&completion),
                )
                .await
                .map_err(|e| check_error(e, GitHubError::UpdateCheck))?
        } else {
            // The in-progress check failed to be created,
            // so fall back to creating the check with a conclusion.
            let full_name = repository_full_name(event_str, event)?;
            let (owner, repo) = split_full_name(full_name)?;
            let head_sha = head_sha(
                event,
                #[cfg(feature = "plus")]
                self.dispatch.as_ref(),
            )?;
            self.github_client(log)?
                .post(
                    format!("/repos/{owner}/{repo}/check-runs"),
                    Some(&NewCompletion {
                        head_sha: &head_sha,
                        completion,
                    }),
                )
                .await
                .map_err(|e| check_error(e, GitHubError::CreateCheck))?
        };
        // The record of the check as GitHub holds it, so every writer records it the same way.
        let CompletedCheck {
            id,
            name,
            conclusion: completed_conclusion,
            external_id: completed_external_id,
            app,
        } = completed;
        let own_check = SectionCheck {
            id: id.into_inner(),
            name,
            conclusion: completed_conclusion.or_else(|| conclusion_name(conclusion)),
            external_id: completed_external_id.unwrap_or(external_id),
        };
        Ok((own_check, app.and_then(Author::new)))
    }

    /// Best-effort: complete a still in-progress check as failed when
    /// `bencher run` errors before the results are posted, so the check
    /// does not linger `in_progress` forever. Failures are logged, never fatal.
    ///
    /// The check keeps whatever name it was started with. Unlike the completed
    /// check, there is no Report to name it after, and the Project name is
    /// precisely what could not be resolved before the run. So a run that both
    /// failed to resolve the Project name and errored before posting results
    /// reports under the bare `Bencher Report` name.
    pub async fn fail_check(&self, handle: &CheckRunHandle, log: bool) {
        let CheckRunHandle { owner, repo, id } = handle;
        let failed = async {
            self.github_client(log)?
                .checks(owner, repo)
                .update_check_run(*id)
                .output(check_run_output(branded_summary(FAILED_SUMMARY)))
                .conclusion(CheckRunConclusion::Failure)
                .send()
                .await
                .map(drop)
                .map_err(|e| check_error(e, GitHubError::UpdateCheck))
        }
        .await;
        if let Err(err) = failed {
            cli_eprintln_quietable!(log, "Failed to complete GitHub Check\n{err}");
        }
    }

    /// The pull request context of a detached run, for the server to post its check and comment.
    #[cfg(feature = "plus")]
    pub fn job_github(&self, log: bool) -> Result<Option<JsonNewJobGitHub>, GitHubError> {
        if !is_github_actions() {
            cli_println_quietable!(
                log,
                "Not running as a GitHub Action. Skipping CI integration.\n{}",
                docker_env(GITHUB_ACTIONS)
            );
            return Ok(None);
        }
        let (event_str, event) = github_event()?;
        Ok(Some(JsonNewJobGitHub {
            repository_id: repository_id(&event_str, &event)?,
            head_sha: head_sha(&event, self.dispatch.as_ref())?
                .parse()
                .map_err(GitHubError::BadSha)?,
            number: self
                .issue_number(&event_str, &event)?
                .map(GitHubIssueNumber::try_from)
                .transpose()
                .map_err(GitHubError::BadNumber)?,
            ci_id: self.ci_id.clone().map(CiId::from),
            ci_only_thresholds: self.ci_only_thresholds.then_some(true),
            ci_only_on_alert: self.ci_only_on_alert.then_some(true),
        }))
    }

    /// On a `repository_dispatch`, the attach continues the detached run whose callback sent it,
    /// and returns that run's comment tag flags. A payload that does not parse is passed over with a warning.
    #[cfg(feature = "plus")]
    pub fn read_dispatch(&mut self, log: bool) -> Option<bencher_comment::SubAdapter> {
        if !is_github_actions()
            || std::env::var(GITHUB_EVENT_NAME).as_deref() != Ok(REPOSITORY_DISPATCH)
        {
            return None;
        }
        let dispatch = github_event()
            .and_then(|(event_str, event)| {
                let full_name = repository_full_name(&event_str, &event)?;
                let (owner, repo) = split_full_name(full_name)?;
                Ok((owner.to_owned(), repo.to_owned(), event))
            })
            .map_err(PayloadError::Event)
            .and_then(|(owner, repo, event)| {
                DispatchPayload::from_event(&event)
                    .map(|payload| (owner, repo, payload))
                    .map_err(PayloadError::Json)
            });
        match dispatch {
            Ok((owner, repo, payload)) => Some(self.adopt(owner, repo, payload)),
            Err(err) => {
                cli_eprintln_quietable!(
                    log,
                    "Warning: failed to read the `repository_dispatch` payload, so it is ignored: {err}"
                );
                None
            },
        }
    }

    /// The payload stands in for `--ci-id` and `--ci-number` where they are not given.
    #[cfg(feature = "plus")]
    fn adopt(
        &mut self,
        owner: String,
        repo: String,
        payload: DispatchPayload,
    ) -> bencher_comment::SubAdapter {
        let DispatchPayload {
            bencher:
                BencherPayload {
                    ci_id,
                    ci_number,
                    build_time,
                    file_size,
                },
            github: GitHubPayload { sha, check },
        } = payload;
        self.ci_id = self.ci_id.take().or(ci_id);
        self.ci_number = self.ci_number.or(ci_number);
        self.dispatch = Some(Dispatched {
            sha,
            check: check.map(|id| CheckRunHandle { owner, repo, id }),
        });
        bencher_comment::SubAdapter {
            build_time,
            file_size,
        }
    }

    /// The check the detached run started, which the attach completes instead of starting one.
    #[cfg(feature = "plus")]
    pub fn adopted_check(&self) -> Option<CheckRunHandle> {
        self.dispatch.as_ref()?.check.clone()
    }

    fn github_client(&self, log: bool) -> Result<Octocrab, GitHubError> {
        let mut builder = Octocrab::builder().user_access_token(self.token.as_ref().to_owned());
        if let Some(url) = github_api_url(log) {
            builder = builder.base_uri(url).map_err(GitHubError::BaseUri)?;
        }
        builder.build().map_err(GitHubError::Auth)
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

        let mut builder = Octocrab::builder().user_access_token(self.token.as_ref().to_owned());
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
        let body = report_comment.html_with_max_length(
            self.ci_only_thresholds,
            self.ci_id.as_deref(),
            MAX_LENGTH,
        );
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
        comment.map(drop).map_err(|e| {
            comment_error(
                e,
                if comment_id.is_some() {
                    GitHubError::UpdateComment
                } else {
                    GitHubError::CreateComment
                },
            )
        })
    }
}

/// A GitHub Check created with an `in_progress` status before the benchmark runs.
/// The check is completed with a conclusion once the results are ready.
#[derive(Debug, Clone)]
pub struct CheckRunHandle {
    owner: String,
    repo: String,
    id: CheckRunId,
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

#[cfg(feature = "plus")]
fn repository_id(
    event_str: &str,
    event: &serde_json::Value,
) -> Result<GitHubRepositoryId, GitHubError> {
    event
        .get("repository")
        .and_then(|repository| repository.get("id"))
        .and_then(serde_json::Value::as_u64)
        .and_then(|id| GitHubRepositoryId::try_from(id).ok())
        .ok_or_else(|| GitHubError::BadRepositoryId(event_str.to_owned()))
}

fn split_full_name(full_name: &str) -> Result<(&str, &str), GitHubError> {
    full_name
        .split_once('/')
        .ok_or_else(|| GitHubError::InvalidFullName(full_name.to_owned()))
}

// Probe the GitHub event payload for the head SHA of the commit under test.
// For `pull_request` and `pull_request_target` events, `GITHUB_SHA` is the
// ephemeral merge commit (or the base branch), so a GitHub Check created on it
// would not appear on the pull request.
// https://docs.github.com/en/webhooks/webhook-events-and-payloads#pull_request
// https://docs.github.com/en/webhooks/webhook-events-and-payloads#workflow_run
fn check_head_sha(event: &serde_json::Value) -> Option<&str> {
    event
        .get("pull_request")
        .and_then(|pull_request| pull_request.get("head"))
        .and_then(|head| head.get("sha"))
        .or_else(|| {
            event
                .get("workflow_run")
                .and_then(|workflow_run| workflow_run.get("head_sha"))
        })
        .and_then(serde_json::Value::as_str)
}

// Resolve the head SHA for the GitHub Check from the event payload,
// falling back to the `GITHUB_SHA` environment variable value.
// The environment variable value is passed in so this stays deterministic in tests.
fn resolve_head_sha(
    event: &serde_json::Value,
    github_sha: Option<String>,
) -> Result<String, GitHubError> {
    if let Some(head_sha) = check_head_sha(event) {
        Ok(head_sha.to_owned())
    } else {
        github_sha.ok_or(GitHubError::NoSha)
    }
}

// The head SHA a `repository_dispatch` names wins, since `GITHUB_SHA` on a dispatch
// is the head of the default branch.
fn head_sha(
    event: &serde_json::Value,
    #[cfg(feature = "plus")] dispatch: Option<&Dispatched>,
) -> Result<String, GitHubError> {
    #[cfg(feature = "plus")]
    if let Some(dispatch) = dispatch {
        return Ok(dispatch.sha.clone().into());
    }
    resolve_head_sha(event, std::env::var(GITHUB_SHA).ok())
}

/// The workflow run attempt that this job is part of on GitHub Actions, whether or not `--github-actions` is set.
pub fn run_attempt_key() -> Option<String> {
    workflow_run_attempt(
        is_github_actions(),
        std::env::var(ACT).as_deref() == Ok("true"),
        std::env::var(GITHUB_REPOSITORY).ok(),
        std::env::var(GITHUB_RUN_ID).ok(),
        std::env::var(GITHUB_RUN_ATTEMPT).ok(),
    )
}

// A rerun is a new attempt, so it resets the branch again.
// `nektos/act` sets `ACT` and defaults the run ID and attempt to 1, so every local run would share one key.
// The environment is read by the caller so this stays deterministic in tests.
fn workflow_run_attempt(
    is_github_actions: bool,
    is_act: bool,
    repository: Option<String>,
    run_id: Option<String>,
    run_attempt: Option<String>,
) -> Option<String> {
    let run_id = run_id.filter(|run_id| !run_id.is_empty())?;
    (is_github_actions && !is_act).then(|| {
        format!(
            "{repository}/{run_id}/{run_attempt}",
            repository = repository.unwrap_or_default(),
            run_attempt = run_attempt.unwrap_or_default(),
        )
    })
}

// A job that ended without results fails the check whatever its report shows.
fn check_conclusion(
    has_alert: bool,
    #[cfg(feature = "plus")] failure: Option<JobFailure>,
) -> CheckRunConclusion {
    #[cfg(feature = "plus")]
    match failure {
        Some(JobFailure::Failed) => return CheckRunConclusion::Failure,
        Some(JobFailure::Canceled) => return CheckRunConclusion::Cancelled,
        None => {},
    }
    if has_alert {
        CheckRunConclusion::Failure
    } else {
        CheckRunConclusion::Success
    }
}

fn conclusion_name(conclusion: CheckRunConclusion) -> Option<String> {
    serde_json::to_value(conclusion)
        .ok()?
        .as_str()
        .map(ToOwned::to_owned)
}

fn check_run_output(summary: String) -> CheckRunOutput {
    CheckRunOutput {
        title: String::new(),
        summary,
        text: None,
        annotations: Vec::new(),
        images: Vec::new(),
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

// https://github.blog/changelog/2023-02-02-github-actions-updating-the-default-github_token-permissions-to-read-only/
fn comment_error(err: octocrab::Error, variant: fn(octocrab::Error) -> GitHubError) -> GitHubError {
    if is_permissions_error(&err) {
        GitHubError::BadCommentPermissions(err)
    } else {
        variant(err)
    }
}

// https://github.blog/changelog/2023-02-02-github-actions-updating-the-default-github_token-permissions-to-read-only/
fn check_error(err: octocrab::Error, variant: fn(octocrab::Error) -> GitHubError) -> GitHubError {
    if is_permissions_error(&err) {
        GitHubError::BadCheckPermissions(err)
    } else {
        variant(err)
    }
}

pub(crate) fn github_api_url(log: bool) -> Option<String> {
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

// https://docs.github.com/en/rest/checks/runs?apiVersion=2022-11-28#update-a-check-run
#[derive(serde::Serialize)]
struct Completion<'a> {
    name: &'a str,
    output: CheckRunOutput,
    conclusion: CheckRunConclusion,
    external_id: &'a str,
}

// https://docs.github.com/en/rest/checks/runs?apiVersion=2022-11-28#create-a-check-run
#[derive(serde::Serialize)]
struct NewCompletion<'a> {
    head_sha: &'a str,
    #[serde(flatten)]
    completion: Completion<'a>,
}

#[derive(serde::Deserialize)]
struct CompletedCheck {
    id: CheckRunId,
    name: String,
    conclusion: Option<String>,
    external_id: Option<String>,
    app: Option<CheckApp>,
}

/// What a `repository_dispatch` carries to its attach. The body also names the job and the
/// project under `bencher`, which only the workflow reads.
#[cfg(feature = "plus")]
#[derive(Debug, serde::Deserialize)]
struct DispatchPayload {
    bencher: BencherPayload,
    github: GitHubPayload,
}

/// The `bencher run` options that the attach takes from the detached run.
#[cfg(feature = "plus")]
#[derive(Debug, serde::Deserialize)]
struct BencherPayload {
    ci_id: Option<String>,
    ci_number: Option<u64>,
    #[serde(default)]
    build_time: bool,
    #[serde(default)]
    file_size: bool,
}

/// The head SHA the attach posts at, and the check it completes.
#[cfg(feature = "plus")]
#[derive(Debug, serde::Deserialize)]
struct GitHubPayload {
    sha: GitHash,
    check: Option<CheckRunId>,
}

/// A `repository_dispatch` payload the attach reads past with a warning.
#[cfg(feature = "plus")]
#[derive(Debug, thiserror::Error)]
enum PayloadError {
    #[error("{0}")]
    Event(GitHubError),
    #[error("{0}")]
    Json(serde_json::Error),
}

/// The check and head SHA the attach takes over from the `repository_dispatch` of a detached run.
#[cfg(feature = "plus")]
#[derive(Debug)]
pub struct Dispatched {
    sha: GitHash,
    check: Option<CheckRunHandle>,
}

#[cfg(feature = "plus")]
impl DispatchPayload {
    fn from_event(event: &serde_json::Value) -> serde_json::Result<Self> {
        Self::deserialize(
            event
                .get("client_payload")
                .unwrap_or(&serde_json::Value::Null),
        )
    }
}

/// Whether `api_url`, the `GITHUB_API_URL` that the comment uses, is github.com's API, which an
/// unset one defaults to: the only API where the Bencher Cloud GitHub App links a repository and
/// posts the check and comment of a detached run.
pub fn is_github_com_api(api_url: Option<&str>) -> bool {
    api_url.is_none_or(|api_url| api_url.trim_end_matches('/') == DEFAULT_GITHUB_API_URL)
}

#[cfg(test)]
mod tests {
    use octocrab::params::checks::CheckRunConclusion;

    use super::{
        FAILED_SUMMARY, GitHubError, branded_summary, check_conclusion, check_head_sha,
        check_run_output, resolve_head_sha, workflow_run_attempt,
    };

    #[test]
    fn check_head_sha_pull_request() {
        let event = serde_json::json!({
            "number": 1234,
            "pull_request": {
                "head": {
                    "ref": "feature",
                    "sha": "f1e2d3c4b5a697887766554433221100ffeeddcc",
                    "repo": { "full_name": "contributor/repo" }
                },
                "base": {
                    "ref": "main",
                    "sha": "0011223344556677889900aabbccddeeff001122"
                }
            },
            "repository": { "full_name": "owner/repo" }
        });
        assert_eq!(
            check_head_sha(&event),
            Some("f1e2d3c4b5a697887766554433221100ffeeddcc")
        );
    }

    #[test]
    fn check_head_sha_workflow_run() {
        let event = serde_json::json!({
            "action": "completed",
            "workflow_run": {
                "event": "pull_request",
                "conclusion": "success",
                "head_branch": "feature",
                "head_sha": "a1b2c3d4e5f60718293a4b5c6d7e8f9012345678"
            },
            "repository": { "full_name": "owner/repo" }
        });
        assert_eq!(
            check_head_sha(&event),
            Some("a1b2c3d4e5f60718293a4b5c6d7e8f9012345678")
        );
    }

    #[test]
    fn check_head_sha_push() {
        let event = serde_json::json!({
            "ref": "refs/heads/main",
            "before": "0011223344556677889900aabbccddeeff001122",
            "after": "f1e2d3c4b5a697887766554433221100ffeeddcc",
            "repository": { "full_name": "owner/repo" }
        });
        assert_eq!(check_head_sha(&event), None);
    }

    #[test]
    fn check_head_sha_non_string() {
        let event = serde_json::json!({
            "pull_request": { "head": { "sha": 1234 } }
        });
        assert_eq!(check_head_sha(&event), None);
    }

    #[test]
    fn resolve_head_sha_pull_request() {
        // The event head SHA wins even when `GITHUB_SHA` is set
        let event = serde_json::json!({
            "pull_request": {
                "head": { "sha": "f1e2d3c4b5a697887766554433221100ffeeddcc" }
            }
        });
        assert_eq!(
            resolve_head_sha(
                &event,
                Some("0011223344556677889900aabbccddeeff001122".to_owned())
            )
            .expect("head SHA"),
            "f1e2d3c4b5a697887766554433221100ffeeddcc"
        );
    }

    #[test]
    fn resolve_head_sha_env_fallback() {
        let event = serde_json::json!({
            "ref": "refs/heads/main",
            "repository": { "full_name": "owner/repo" }
        });
        assert_eq!(
            resolve_head_sha(
                &event,
                Some("0011223344556677889900aabbccddeeff001122".to_owned())
            )
            .expect("env SHA"),
            "0011223344556677889900aabbccddeeff001122"
        );
    }

    #[test]
    fn workflow_run_attempt_names_the_run_attempt() {
        let first = attempt(true, Some("1234"), "1");
        assert!(first.is_some());
        assert_ne!(first, attempt(true, Some("1234"), "2"));
        assert_ne!(first, attempt(true, Some("5678"), "1"));
        assert_ne!(
            first,
            workflow_run_attempt(
                true,
                false,
                Some("owner/other".to_owned()),
                Some("1234".to_owned()),
                Some("1".to_owned())
            )
        );
    }

    #[test]
    fn workflow_run_attempt_needs_github_actions_and_a_run_id() {
        assert_eq!(attempt(false, Some("1234"), "1"), None);
        assert_eq!(attempt(true, None, "1"), None);
        assert_eq!(attempt(true, Some(""), "1"), None);
    }

    #[test]
    fn workflow_run_attempt_skips_act() {
        assert_eq!(
            workflow_run_attempt(
                true,
                true,
                Some("owner/repo".to_owned()),
                Some("1".to_owned()),
                Some("1".to_owned())
            ),
            None
        );
    }

    fn attempt(is_github_actions: bool, run_id: Option<&str>, run_attempt: &str) -> Option<String> {
        workflow_run_attempt(
            is_github_actions,
            false,
            Some("owner/repo".to_owned()),
            run_id.map(ToOwned::to_owned),
            Some(run_attempt.to_owned()),
        )
    }

    #[test]
    fn resolve_head_sha_missing() {
        let event = serde_json::json!({
            "ref": "refs/heads/main",
            "repository": { "full_name": "owner/repo" }
        });
        assert!(matches!(
            resolve_head_sha(&event, None),
            Err(GitHubError::NoSha)
        ));
    }

    #[test]
    fn check_conclusion_alert() {
        assert!(matches!(
            check_conclusion(
                true,
                #[cfg(feature = "plus")]
                None
            ),
            CheckRunConclusion::Failure
        ));
    }

    #[test]
    fn check_conclusion_no_alert() {
        assert!(matches!(
            check_conclusion(
                false,
                #[cfg(feature = "plus")]
                None
            ),
            CheckRunConclusion::Success
        ));
    }

    #[test]
    fn branded_summary_failed() {
        let summary = branded_summary(FAILED_SUMMARY);
        assert!(summary.starts_with("<h2>"));
        assert!(summary.contains("Bencher Report"));
        assert!(summary.ends_with(FAILED_SUMMARY));
    }

    #[test]
    fn check_run_output_shape() {
        let output = check_run_output("Benchmarks are running.".to_owned());
        assert_eq!(output.title, "");
        assert_eq!(output.summary, "Benchmarks are running.");
        assert!(output.text.is_none());
        assert!(output.annotations.is_empty());
        assert!(output.images.is_empty());
    }

    #[test]
    fn octocrab_pull_request_accepts_minimal_check_run_payload() {
        use octocrab::models::pulls::PullRequest;

        // GitHub's check-run response `pull_requests` items are the minimal/simple PR
        // representation (no `node_id`, etc.). octocrab >= 0.52 models these as optional.
        // Guards against an octocrab downgrade reintroducing
        // https://github.com/bencherdev/bencher/issues/894
        let minimal = serde_json::json!([{
            "url": "https://api.github.com/repos/owner/repo/pulls/1",
            "id": 1934,
            "number": 1,
            "head": {
                "ref": "feature",
                "sha": "f1e2d3c4b5a697887766554433221100ffeeddcc",
                "repo": { "id": 1, "url": "https://api.github.com/repos/owner/repo", "name": "repo" }
            },
            "base": {
                "ref": "main",
                "sha": "0011223344556677889900aabbccddeeff001122",
                "repo": { "id": 1, "url": "https://api.github.com/repos/owner/repo", "name": "repo" }
            }
        }]);
        let parsed: Result<Vec<PullRequest>, _> = serde_json::from_value(minimal);
        assert!(
            parsed.is_ok(),
            "octocrab PullRequest must accept GitHub's minimal check-run payload: {parsed:?}"
        );
    }
}

#[cfg(test)]
#[cfg(feature = "plus")]
mod job_tests {
    use octocrab::params::checks::CheckRunConclusion;

    use super::{JobFailure, check_conclusion};

    #[test]
    fn a_job_failure_sets_the_conclusion_whatever_the_alerts() {
        for has_alert in [true, false] {
            assert!(
                matches!(
                    check_conclusion(has_alert, Some(JobFailure::Failed)),
                    CheckRunConclusion::Failure
                ),
                "{has_alert}"
            );
            assert!(
                matches!(
                    check_conclusion(has_alert, Some(JobFailure::Canceled)),
                    CheckRunConclusion::Cancelled
                ),
                "{has_alert}"
            );
        }
        assert!(matches!(
            check_conclusion(true, None),
            CheckRunConclusion::Failure
        ));
        assert!(matches!(
            check_conclusion(false, None),
            CheckRunConclusion::Success
        ));
    }
}
