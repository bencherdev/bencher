use std::{collections::BTreeMap, future::Future, time::Duration};

use bencher_callback::{CallbackAttempt, CallbackCredential};
use bencher_comment::{
    GitHubApi as CommentApi, GroupComment, IN_PROGRESS_SUMMARY, MAX_LENGTH, ReportComment, Section,
    SectionCheck, SubAdapter, branded_summary, check_run_id, check_run_name,
};
use bencher_github_client::{GitHubApiError, GitHubApp, GitHubInstallationToken};
use bencher_github_post::{Check, CommentId, CommentsPage, Conclusion, GitHubPost, Head, Updated};
use bencher_json::{
    CiId, GitHubCheckRunId, GitHubIssueNumber, JobStatus, JobUuid, JsonJobConfig, OrganizationUuid,
    ProjectSlug, ProjectUuid, ResourceName,
    runner::{JobGitHubFailure, JobGitHubState},
};
use diesel::{
    ExpressionMethods as _, OptionalExtension as _, QueryDsl as _, RunQueryDsl as _,
    SelectableHelper as _, result::QueryResult,
};
use http::StatusCode;
use slog::Logger;
use tokio::time::Instant;

use super::{
    super::Kind, Claim, Delivery, MAX_ATTEMPTS, NoToken, NoTokenCause, Stage, Unprepared, Verdict,
    answered, millis, response_status, verdict, wait_before,
};
use crate::{
    model::{
        organization::license::any_valid_license,
        project::{ProjectId, linked_repository::QueryLinkedRepository, report::QueryReport},
        runner::{JobId, PostOutcome, QueryJobGitHub},
    },
    schema,
};

/// What a post's comment names as the source of its links.
const SOURCE: &str = "github";

/// Start the job's check, or post it once the job has finished: for each job, one at a time, so a
/// post never begins while its start creates the check.
pub(in super::super) async fn advance(
    delivery: &Delivery,
    log: &Logger,
    job_id: JobId,
    kind: Kind,
) {
    Poster(delivery).advance(log, job_id, kind).await;
}

/// Posts a job's GitHub check and comment with what every delivery task shares.
struct Poster<'d>(&'d Delivery);

impl Poster<'_> {
    async fn advance(&self, log: &Logger, job_id: JobId, kind: Kind) {
        let Some(_job) = self
            .0
            .shutdown
            .run_until_cancelled(self.0.jobs.lock(job_id))
            .await
        else {
            return;
        };
        let Some((post, report)) = self.load(log, job_id, kind).await else {
            return;
        };
        if !self.licensed(log, &post).await {
            return;
        }
        if let Some(finish) = post.finish {
            self.post(log, post, finish, report).await;
        } else if post.github.check_run_id.is_none() {
            self.start(log, &post).await;
        }
    }

    /// The job's pending post, read again while the database cannot be read. A post counts each
    /// failed read as an attempt, and the last one settles it; a start leaves it to the post that
    /// the job's end fires.
    async fn load(&self, log: &Logger, job_id: JobId, kind: Kind) -> Option<(Post, QueryReport)> {
        let mut unread = 0;
        loop {
            if let Ok(loaded) = self.load_post(log, job_id).await {
                return loaded;
            }
            if kind == Kind::Start {
                return None;
            }
            unread += 1;
            let outcome = unposted(&Verdict::Retry, unread);
            let last = outcome.is_some();
            self.count_unread(log, job_id, outcome).await;
            if last {
                return None;
            }
            if let Some(wait) = wait_before(unread + 1)
                && self
                    .0
                    .shutdown
                    .run_until_cancelled(tokio::time::sleep(wait))
                    .await
                    .is_none()
            {
                return None;
            }
        }
    }

    /// The job's pending post, and the job's report.
    async fn load_post(
        &self,
        log: &Logger,
        job_id: JobId,
    ) -> QueryResult<Option<(Post, QueryReport)>> {
        let row = schema::job_github::table
            .inner_join(
                schema::job::table
                    .inner_join(schema::report::table.inner_join(
                        schema::project::table.inner_join(schema::organization::table),
                    )),
            )
            .filter(schema::job_github::job_id.eq(job_id))
            .filter(schema::job_github::state.eq(JobGitHubState::Pending))
            .select((
                QueryJobGitHub::as_select(),
                schema::job::uuid,
                schema::job::status,
                schema::job::config,
                QueryReport::as_select(),
                schema::project::id,
                schema::project::uuid,
                schema::project::name,
                schema::project::slug,
                schema::organization::uuid,
            ))
            .first::<(
                QueryJobGitHub,
                JobUuid,
                JobStatus,
                JsonJobConfig,
                QueryReport,
                ProjectId,
                ProjectUuid,
                ResourceName,
                ProjectSlug,
                OrganizationUuid,
            )>(&mut *self.0.connection.lock().await)
            .optional()
            .inspect_err(|e| {
                slog::error!(log, "Failed to load a GitHub post"; "job_id" => ?job_id, "error" => %e);
            })?;
        let Some((
            github,
            job_uuid,
            job_status,
            config,
            report,
            project_id,
            project,
            project_name,
            project_slug,
            organization,
        )) = row
        else {
            return Ok(None);
        };
        let claim = Claim {
            job_id,
            job_uuid,
            organization,
            project_id,
            project,
            attempts: github.attempts,
            claimed: Instant::now(),
        };
        let post = Post {
            claim,
            github,
            finish: Finish::of(job_status),
            sub_adapter: sub_adapter(&config),
            project_name,
            project_slug,
        };
        Ok(Some((post, report)))
    }

    /// Whether the server may post to its GitHub. An unlicensed post stays pending, so a restart
    /// with the license back posts it.
    async fn licensed(&self, log: &Logger, post: &Post) -> bool {
        let Some(github) = self.0.github.as_ref() else {
            return true;
        };
        let host = github.app.host();
        if host.is_github_com() {
            return true;
        }
        let job = post.claim.job_uuid;
        let Some(mut conn) = self
            .0
            .shutdown
            .run_until_cancelled(self.0.read_connection(log, job))
            .await
        else {
            return false;
        };
        match any_valid_license(&mut conn, &github.licensor, self.0.clock.now()) {
            Ok(true) => true,
            Ok(false) => {
                slog::warn!(log, "GitHub post skipped: a GitHub Enterprise Server needs a Bencher Plus Enterprise license"; "job" => %job, "host" => %host);
                false
            },
            Err(e) => {
                slog::error!(log, "GitHub post skipped: failed to read the server's licenses"; "job" => %job, "error" => %e);
                false
            },
        }
    }

    /// Create the job's check in progress. A start that fails leaves the post to create the check
    /// with its conclusion, unless no post could ever succeed.
    async fn start(&self, log: &Logger, post: &Post) {
        let Some(app) = self.0.github_app() else {
            return;
        };
        let link = match self.post_link(log, post).await {
            Ok(Some(link)) => link,
            Ok(None) => {
                self.settle(
                    log,
                    post,
                    PostOutcome::failed(JobGitHubFailure::Unlinked),
                    None,
                )
                .await;
                return;
            },
            Err(_) => return,
        };
        let credential = credential(app, &link);
        let token = match self.mint_turn(log, post, app, link, &credential).await {
            Some(Ok(token)) => token,
            Some(Err(Ended::Failed(status, outcome))) => {
                self.settle(log, post, outcome, status).await;
                return;
            },
            Some(Err(_)) | None => return,
        };
        let name = post.check_name(post.github.ci_id.as_ref());
        let summary = branded_summary(IN_PROGRESS_SUMMARY);
        let check = Check {
            name: &name,
            summary: &summary,
            conclusion: None,
        };
        let api = GitHubPost::new(app, &token.token, &token.full_name);
        let created = self
            .request(
                log,
                post,
                &credential,
                Stage::Check,
                api.create_check(&post.github.head_sha, &check),
            )
            .await;
        drop(token);
        if let Some(Ok(check_run_id)) = created {
            self.record_check(log, post, check_run_id).await;
        }
    }

    /// Complete the check, then write the comment, in up to 3 attempts. A check that one attempt
    /// completed is not completed again by the next, and a report that one attempt could not read
    /// is read again by the next.
    async fn post(&self, log: &Logger, mut post: Post, finish: Finish, report: QueryReport) {
        let Some(app) = self.0.github_app().cloned() else {
            self.settle(
                log,
                &post,
                PostOutcome::failed(JobGitHubFailure::Unlinked),
                None,
            )
            .await;
            return;
        };
        let mut writer = None;
        let mut checked = false;
        loop {
            let number = post.claim.attempts + 1;
            // A count at the limit, as a restart can find, never posts again.
            if number > MAX_ATTEMPTS {
                self.settle(
                    log,
                    &post,
                    PostOutcome::failed(JobGitHubFailure::Exhausted),
                    None,
                )
                .await;
                return;
            }
            if let Some(wait) = wait_before(number)
                && self
                    .0
                    .shutdown
                    .run_until_cancelled(tokio::time::sleep(wait))
                    .await
                    .is_none()
            {
                return;
            }
            if writer.is_none() {
                let built = self
                    .comment(
                        log,
                        post.claim.job_uuid,
                        report.clone(),
                        &post.sub_adapter,
                        Some(finish),
                    )
                    .await;
                match built {
                    Ok(built) => writer = Some(built),
                    Err(Unprepared::Shutdown) => return,
                    Err(_) => {},
                }
            }
            let ended = if let Some(writer) = &writer {
                self.post_attempt(log, &mut post, finish, &app, writer, &mut checked)
                    .await
            } else {
                Some(Ended::Unread)
            };
            let Some(ended) = ended else {
                return;
            };
            post.claim.attempts = number;
            let (status, outcome) = match ended {
                Ended::Posted => (None, Some(PostOutcome::Posted)),
                Ended::Failed(status, outcome) => (status, Some(outcome)),
                Ended::Unread => (None, unposted(&Verdict::Retry, number)),
                Ended::MintRefused(attempt) => (
                    response_status(&attempt),
                    unposted(&verdict(&attempt), number),
                ),
                Ended::Refused(attempt) => (
                    response_status(&attempt),
                    unposted(&sent_verdict(&attempt), number),
                ),
            };
            let recorded = QueryJobGitHub::record_attempt(
                &mut *self.0.connection.lock().await,
                post.claim.job_id,
                status,
                outcome.clone(),
                self.0.clock.now(),
            );
            if !self.settled(log, &post, recorded, outcome, status) {
                return;
            }
        }
    }

    /// One attempt: the check, then the comment while the pull request's lock is held. `None`
    /// once shutdown cuts it short.
    async fn post_attempt(
        &self,
        log: &Logger,
        post: &mut Post,
        finish: Finish,
        app: &GitHubApp,
        writer: &ReportComment,
        checked: &mut bool,
    ) -> Option<Ended> {
        let link = match self.post_link(log, post).await {
            Ok(Some(link)) => link,
            Ok(None) => {
                return Some(Ended::Failed(
                    None,
                    PostOutcome::failed(JobGitHubFailure::Unlinked),
                ));
            },
            Err(_) => return Some(Ended::Unread),
        };
        let credential = credential(app, &link);
        let token = match self.mint_turn(log, post, app, link, &credential).await? {
            Ok(token) => token,
            Err(ended) => return Some(ended),
        };
        let api = GitHubPost::new(app, &token.token, &token.full_name);
        if !*checked {
            let completed = self
                .complete_check(log, post, finish, &api, &credential, writer)
                .await?;
            if let Err(attempt) = completed {
                return Some(Ended::Refused(attempt));
            }
            *checked = true;
        }
        let Some(number) = post.github.number else {
            return Some(Ended::Posted);
        };
        self.write_comment(log, post, &api, &credential, writer, number)
            .await
    }

    /// Write the project's group comment on the pull request while its lock is held, unless the
    /// pull request's head has moved or a newer job holds the post's own section.
    async fn write_comment(
        &self,
        log: &Logger,
        post: &Post,
        api: &GitHubPost<'_>,
        credential: &CallbackCredential,
        writer: &ReportComment,
        number: GitHubIssueNumber,
    ) -> Option<Ended> {
        let _pull_request = self
            .0
            .shutdown
            .run_until_cancelled(
                self.0
                    .pull_requests
                    .lock((post.github.repository_id, number)),
            )
            .await?;
        let head = self
            .request(
                log,
                post,
                credential,
                Stage::Head,
                api.head(number, &post.github.head_sha),
            )
            .await?;
        match head {
            Ok(Head::Current) => {},
            Ok(Head::Moved | Head::NoPullRequest) => return Some(Ended::Posted),
            Err(attempt) => return Some(Ended::Refused(attempt)),
        }
        let own_key = SectionKey::new(post.github.ci_id.as_ref(), writer);
        let others = match self.sections(log, post, number, &own_key).await? {
            Sections::Others(others) => others,
            Sections::Held => {
                slog::info!(log, "A newer job of the section is on the commit, so the post writes no comment"; "job" => %post.claim.job_uuid);
                return Some(Ended::Posted);
            },
            Sections::Unread => return Some(Ended::Unread),
        };
        let own_check = post.section_check(
            post.github.ci_id.as_ref(),
            post.github.check_run_id,
            post.finish,
            writer,
        );
        let group = GroupComment::new(
            own_key.ci_id(),
            own_check,
            writer,
            others
                .iter()
                .map(|other| {
                    Section::new(other.key.ci_id(), other.check.clone(), Some(&other.report))
                })
                .collect(),
            post.github.head_sha.clone(),
        );
        let require_threshold = post.github.ci_only_thresholds;
        if require_threshold && !group.has_threshold() {
            return Some(Ended::Posted);
        }
        let body = group.html(require_threshold, MAX_LENGTH);
        let found = match self
            .group_comment(log, post, api, credential, number)
            .await?
        {
            Ok(found) => found,
            Err(attempt) => return Some(Ended::Refused(attempt)),
        };
        if let Some(id) = found {
            let updated = self
                .request(
                    log,
                    post,
                    credential,
                    Stage::Comment,
                    api.update_comment(id, &body),
                )
                .await?;
            match updated {
                Ok(Updated::Updated) => return Some(Ended::Posted),
                // Deleted since it was read, so a new comment follows the rule for one.
                Ok(Updated::Gone) => {},
                Err(attempt) => return Some(Ended::Refused(attempt)),
            }
        }
        if post.github.ci_only_on_alert && !group.has_alert() {
            return Some(Ended::Posted);
        }
        let created = self
            .request(
                log,
                post,
                credential,
                Stage::Comment,
                api.create_comment(number, &body),
            )
            .await?;
        Some(match created {
            Ok(()) => Ended::Posted,
            Err(attempt) => Ended::Refused(attempt),
        })
    }

    /// Complete the check that the start created, or create it with its conclusion.
    async fn complete_check(
        &self,
        log: &Logger,
        post: &mut Post,
        finish: Finish,
        api: &GitHubPost<'_>,
        credential: &CallbackCredential,
        writer: &ReportComment,
    ) -> Option<Result<(), CallbackAttempt>> {
        let name = post.check_name(post.github.ci_id.as_ref());
        let summary = writer.html_with_max_length(
            post.github.ci_only_thresholds,
            post.github.ci_id.as_ref().map(AsRef::as_ref),
            MAX_LENGTH,
        );
        let check = Check {
            name: &name,
            summary: &summary,
            conclusion: Some(finish.conclusion(writer)),
        };
        if let Some(check_run_id) = post.github.check_run_id {
            return self
                .request(
                    log,
                    post,
                    credential,
                    Stage::Check,
                    api.update_check(check_run_id, &check),
                )
                .await;
        }
        let created = self
            .request(
                log,
                post,
                credential,
                Stage::Check,
                api.create_check(&post.github.head_sha, &check),
            )
            .await?;
        if let Ok(check_run_id) = &created {
            post.github.check_run_id = Some(*check_run_id);
            self.record_check(log, post, *check_run_id).await;
        }
        Some(created.map(drop))
    }

    /// The project's group comment on the pull request by the app's bot, page by page.
    async fn group_comment(
        &self,
        log: &Logger,
        post: &Post,
        api: &GitHubPost<'_>,
        credential: &CallbackCredential,
        number: GitHubIssueNumber,
    ) -> Option<Result<Option<CommentId>, CallbackAttempt>> {
        let mut page = 1;
        loop {
            let listed = self
                .request(
                    log,
                    post,
                    credential,
                    Stage::Comments,
                    api.comments(number, page, &post.project_slug),
                )
                .await?;
            match listed {
                Ok(CommentsPage {
                    group: Some(id), ..
                }) => return Some(Ok(Some(id))),
                Ok(CommentsPage { group: None, more }) => {
                    if !more {
                        return Some(Ok(None));
                    }
                    page += 1;
                },
                Err(attempt) => return Some(Err(attempt)),
            }
        }
    }

    /// Every other job of the project on the pull request's head SHA, each the newest of its
    /// section, read from the database. `None` once shutdown cuts it short.
    async fn sections(
        &self,
        log: &Logger,
        post: &Post,
        number: GitHubIssueNumber,
        own_key: &SectionKey,
    ) -> Option<Sections> {
        let rows = schema::job_github::table
            .inner_join(schema::job::table.inner_join(schema::report::table))
            .filter(schema::job::project_id.eq(post.claim.project_id))
            .filter(schema::job_github::repository_id.eq(post.github.repository_id))
            .filter(schema::job_github::number.eq(number))
            .filter(schema::job_github::head_sha.eq(&post.github.head_sha))
            .filter(schema::job_github::job_id.ne(post.claim.job_id))
            .order(schema::job_github::job_id.asc())
            .select((
                schema::job_github::job_id,
                schema::job::uuid,
                schema::job_github::ci_id,
                schema::job_github::check_run_id,
                schema::job::status,
                schema::job::config,
                QueryReport::as_select(),
            ))
            .load::<SectionRow>(&mut *self.0.connection.lock().await);
        let rows = match rows {
            Ok(rows) => rows,
            Err(e) => {
                slog::error!(log, "Failed to read the jobs of a pull request"; "job" => %post.claim.job_uuid, "error" => %e);
                return Some(Sections::Unread);
            },
        };
        let mut by_key = BTreeMap::new();
        for (job_id, job_uuid, ci_id, check_run_id, status, config, report) in rows {
            let finish = Finish::of(status);
            let report = match self
                .comment(log, job_uuid, report, &sub_adapter(&config), finish)
                .await
            {
                Ok(report) => report,
                Err(Unprepared::Shutdown) => return None,
                Err(_) => return Some(Sections::Unread),
            };
            let key = SectionKey::new(ci_id.as_ref(), &report);
            let check = post.section_check(ci_id.as_ref(), check_run_id, finish, &report);
            // In ID order, so the newest job of each section stays.
            by_key.insert(
                key.clone(),
                OtherSection {
                    job_id,
                    key,
                    check,
                    report,
                },
            );
        }
        if by_key
            .remove(own_key)
            .is_some_and(|newest| newest.job_id > post.claim.job_id)
        {
            return Some(Sections::Held);
        }
        Some(Sections::Others(by_key.into_values().collect()))
    }

    /// A job's report as its section shows it, pending until the job has finished.
    async fn comment(
        &self,
        log: &Logger,
        job_uuid: JobUuid,
        report: QueryReport,
        sub_adapter: &SubAdapter,
        finish: Option<Finish>,
    ) -> Result<ReportComment, Unprepared> {
        let report = self.0.report(log, job_uuid, report).await?;
        let sub_adapter = SubAdapter {
            build_time: sub_adapter.build_time,
            file_size: sub_adapter.file_size,
        };
        Ok(ReportComment::new(
            self.0.console_url.clone(),
            report,
            sub_adapter,
            SOURCE.to_owned(),
        )
        .with_pending(finish.is_none())
        .with_github_api(CommentApi::GitHubCom))
    }

    /// The project's connected link, only while it is to the job's repository.
    async fn post_link(
        &self,
        log: &Logger,
        post: &Post,
    ) -> QueryResult<Option<QueryLinkedRepository>> {
        let link = self.0.connected_link(log, &post.claim).await?;
        Ok(link.filter(|link| link.repository_id == post.github.repository_id))
    }

    /// Mint a post's token in the installation's turn, and keep the repository's current full
    /// name. A refusal for want of approval looks up the installation's page in the same turn.
    async fn mint_turn(
        &self,
        log: &Logger,
        post: &Post,
        app: &GitHubApp,
        link: QueryLinkedRepository,
        credential: &CallbackCredential,
    ) -> Option<Result<GitHubInstallationToken, Ended>> {
        let turn = self
            .0
            .shutdown
            .run_until_cancelled(self.0.governor.turn(credential))
            .await??;
        let started = Instant::now();
        let minted = self
            .0
            .shutdown
            .run_until_cancelled(self.0.mint(log, &post.claim, app, link))
            .await?;
        let NoToken {
            attempt,
            stage,
            cause,
        } = match minted {
            Ok((link, token)) => {
                let attempt = CallbackAttempt::Delivered(StatusCode::OK);
                let pause = turn.settle(&attempt, self.0.clock.timestamp());
                log_request(
                    log,
                    &post.claim,
                    Stage::Mint,
                    &attempt,
                    started.elapsed(),
                    pause,
                );
                let written =
                    link.write_full_name(&mut *self.0.connection.lock().await, &token.full_name);
                if let Err(e) = written {
                    slog::error!(log, "Failed to write a linked repository's full name"; "job" => %post.claim.job_uuid, "error" => %e);
                }
                return Some(Ok(token));
            },
            Err(no_token) => no_token,
        };
        let approval_url = if let NoTokenCause::Approval(installation) = cause {
            let page = self
                .0
                .shutdown
                .run_until_cancelled(app.installation_url(self.0.clock.now(), installation))
                .await?;
            page.inspect_err(|e| {
                slog::warn!(log, "Failed to look up an installation's page"; "job" => %post.claim.job_uuid, "error" => %e);
            })
            .ok()
            .map(Into::into)
        } else {
            None
        };
        let pause = turn.settle(&attempt, self.0.clock.timestamp());
        log_request(log, &post.claim, stage, &attempt, started.elapsed(), pause);
        #[cfg(feature = "sentry")]
        self.0.alarm(&post.claim, credential, &attempt);
        let status = response_status(&attempt);
        Some(Err(match cause {
            NoTokenCause::Approval(_) => Ended::Failed(
                status,
                PostOutcome::Failed {
                    failure: JobGitHubFailure::Approval,
                    approval_url,
                },
            ),
            NoTokenCause::Disconnected => {
                Ended::Failed(status, PostOutcome::failed(JobGitHubFailure::Unlinked))
            },
            NoTokenCause::Other => Ended::MintRefused(attempt),
        }))
    }

    /// One request in the installation's turn. `None` once shutdown cuts it short, which writes
    /// nothing.
    async fn request<T, F>(
        &self,
        log: &Logger,
        post: &Post,
        credential: &CallbackCredential,
        stage: Stage,
        send: F,
    ) -> Option<Result<T, CallbackAttempt>>
    where
        F: Future<Output = Result<T, GitHubApiError>>,
    {
        let turn = self
            .0
            .shutdown
            .run_until_cancelled(self.0.governor.turn(credential))
            .await??;
        let started = Instant::now();
        let answer = self
            .0
            .shutdown
            .run_until_cancelled(send)
            .await?
            .map_err(answered);
        let delivered = CallbackAttempt::Delivered(StatusCode::OK);
        let attempt = answer.as_ref().err().unwrap_or(&delivered);
        let pause = turn.settle(attempt, self.0.clock.timestamp());
        log_request(log, &post.claim, stage, attempt, started.elapsed(), pause);
        #[cfg(feature = "sentry")]
        self.0.alarm(&post.claim, credential, attempt);
        Some(answer)
    }

    async fn record_check(&self, log: &Logger, post: &Post, check_run_id: GitHubCheckRunId) {
        let recorded = QueryJobGitHub::record_check(
            &mut *self.0.connection.lock().await,
            post.claim.job_id,
            check_run_id,
            self.0.clock.now(),
        );
        if let Err(e) = recorded {
            slog::error!(log, "Failed to record a GitHub check"; "job" => %post.claim.job_uuid, "error" => %e);
        }
    }

    /// Count an attempt of a post that could not be read, and settle it with the outcome, if any.
    async fn count_unread(&self, log: &Logger, job_id: JobId, outcome: Option<PostOutcome>) {
        let recorded = QueryJobGitHub::record_attempt(
            &mut *self.0.connection.lock().await,
            job_id,
            None,
            outcome.clone(),
            self.0.clock.now(),
        );
        match recorded {
            Ok(written) => {
                if written && let Some(PostOutcome::Failed { failure, .. }) = outcome {
                    slog::warn!(log, "GitHub post failed"; "job_id" => ?job_id, "failure" => ?failure);
                }
            },
            Err(e) => {
                slog::error!(log, "Failed to write a GitHub post"; "job_id" => ?job_id, "error" => %e);
            },
        }
    }

    /// Settle the post without an attempt.
    async fn settle(
        &self,
        log: &Logger,
        post: &Post,
        outcome: PostOutcome,
        status: Option<StatusCode>,
    ) {
        let finished = QueryJobGitHub::finish(
            &mut *self.0.connection.lock().await,
            post.claim.job_id,
            outcome.clone(),
            status,
            self.0.clock.now(),
        );
        self.settled(log, post, finished, Some(outcome), status);
    }

    /// Log a write to a pending post, and whether it settled it; true when an attempt was
    /// recorded and the post goes on. A failed post tells Sentry, once an hour per organization.
    fn settled(
        &self,
        log: &Logger,
        post: &Post,
        written: QueryResult<bool>,
        outcome: Option<PostOutcome>,
        status: Option<StatusCode>,
    ) -> bool {
        let claim = &post.claim;
        let latency_ms = millis(claim.claimed.elapsed());
        match (written, outcome) {
            (Ok(true), None) => return true,
            (Ok(true), Some(PostOutcome::Posted)) => {
                slog::info!(log, "GitHub post posted"; "job" => %claim.job_uuid, "organization" => %claim.organization, "project" => %claim.project, "attempts" => claim.attempts, "latency_ms" => latency_ms);
            },
            (Ok(true), Some(PostOutcome::Failed { failure, .. })) => {
                slog::warn!(log, "GitHub post failed"; "job" => %claim.job_uuid, "organization" => %claim.organization, "project" => %claim.project, "attempts" => claim.attempts, "failure" => ?failure, "status" => status.map(|status| status.as_u16()), "latency_ms" => latency_ms);
                #[cfg(feature = "sentry")]
                if self.0.post_alarms.due(&claim.organization) {
                    sentry::capture_error(&PostFailed {
                        failure,
                        status,
                        job: claim.job_uuid,
                        organization: claim.organization,
                        project: claim.project,
                    });
                }
            },
            (Ok(false), _) => {
                slog::warn!(log, "GitHub post is no longer pending"; "job" => %claim.job_uuid);
            },
            (Err(e), _) => {
                slog::error!(log, "Failed to write a GitHub post"; "job" => %claim.job_uuid, "error" => %e);
            },
        }
        false
    }
}

/// A pending post, and the job and project it is for.
struct Post {
    claim: Claim,
    github: QueryJobGitHub,
    /// How the job ended, if it has.
    finish: Option<Finish>,
    sub_adapter: SubAdapter,
    project_name: ResourceName,
    project_slug: ProjectSlug,
}

/// How a job ended.
#[derive(Debug, Clone, Copy)]
enum Finish {
    Processed,
    Failed,
    Canceled,
}

/// How one attempt of a post ended.
enum Ended {
    Posted,
    /// The mint, or its lookup, was refused or never answered.
    MintRefused(CallbackAttempt),
    /// A request with the minted token was refused, or never answered.
    Refused(CallbackAttempt),
    /// The database could not be read.
    Unread,
    /// No attempt can post it.
    Failed(Option<StatusCode>, PostOutcome),
}

/// What keys a section of the group comment: `--ci-id`, else the report's branch, testbed, and
/// adapter. Keys of the two kinds never collide.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum SectionKey {
    CiId(String),
    Tag(String),
}

/// What the post's group comment holds besides the post's own section.
enum Sections {
    Others(Vec<OtherSection>),
    /// A newer job holds the post's own section.
    Held,
    /// The database could not be read.
    Unread,
}

/// Another job's section.
struct OtherSection {
    job_id: JobId,
    key: SectionKey,
    check: SectionCheck,
    report: ReportComment,
}

type SectionRow = (
    JobId,
    JobUuid,
    Option<CiId>,
    Option<GitHubCheckRunId>,
    JobStatus,
    JsonJobConfig,
    QueryReport,
);

/// A post that failed, named by its job and never by its token.
#[cfg(feature = "sentry")]
#[derive(Debug, thiserror::Error)]
#[error(
    "GitHub post failed ({failure:?}, status {status:?}) for job {job}, organization {organization}, project {project}"
)]
struct PostFailed {
    failure: JobGitHubFailure,
    status: Option<StatusCode>,
    job: JobUuid,
    organization: OrganizationUuid,
    project: ProjectUuid,
}

impl Post {
    /// The check's name, as the CLI names it.
    fn check_name(&self, ci_id: Option<&CiId>) -> String {
        check_run_name(check_run_id(
            ci_id.map(AsRef::as_ref),
            Some(&self.project_name),
        ))
    }

    /// A job's check as its section records it. The post's checks carry no `external_id`, and a
    /// check not created yet has no ID.
    fn section_check(
        &self,
        ci_id: Option<&CiId>,
        check_run_id: Option<GitHubCheckRunId>,
        finish: Option<Finish>,
        report: &ReportComment,
    ) -> SectionCheck {
        SectionCheck {
            id: check_run_id.map(u64::from).unwrap_or_default(),
            name: self.check_name(ci_id),
            conclusion: finish.map(|finish| finish.conclusion(report).as_str().to_owned()),
            external_id: String::new(),
        }
    }
}

impl Finish {
    fn of(status: JobStatus) -> Option<Self> {
        match status {
            JobStatus::Processed => Some(Self::Processed),
            JobStatus::Failed => Some(Self::Failed),
            JobStatus::Canceled => Some(Self::Canceled),
            JobStatus::Pending
            | JobStatus::Claimed
            | JobStatus::Running
            | JobStatus::Completed
            | JobStatus::Unknown => None,
        }
    }

    /// A job that ended without results fails the check whatever its report shows.
    fn conclusion(self, report: &ReportComment) -> Conclusion {
        match self {
            Self::Processed => {
                if report.has_alert() {
                    Conclusion::Failure
                } else {
                    Conclusion::Success
                }
            },
            Self::Failed => Conclusion::Failure,
            Self::Canceled => Conclusion::Cancelled,
        }
    }
}

impl SectionKey {
    fn new(ci_id: Option<&CiId>, report: &ReportComment) -> Self {
        if let Some(ci_id) = ci_id {
            Self::CiId(ci_id.to_string())
        } else {
            Self::Tag(report.tag_id(None))
        }
    }

    /// The `--ci-id` that the comment labels the section with.
    fn ci_id(&self) -> Option<String> {
        match self {
            Self::CiId(ci_id) => Some(ci_id.clone()),
            Self::Tag(_) => None,
        }
    }
}

/// How an attempt that did not post settles the post, if it does.
fn unposted(verdict: &Verdict, number: i32) -> Option<PostOutcome> {
    if !matches!(verdict, Verdict::Retry) {
        return Some(PostOutcome::failed(JobGitHubFailure::Refused));
    }
    // The last attempt settles the post itself, so the failure it logs and tells Sentry keeps
    // GitHub's last answer.
    (number >= MAX_ATTEMPTS).then(|| PostOutcome::failed(JobGitHubFailure::Exhausted))
}

/// An installation token can expire while its request waits for the installation's turn, and the
/// next attempt mints a new one.
fn sent_verdict(attempt: &CallbackAttempt) -> Verdict {
    if let CallbackAttempt::Refused(status, _) = attempt
        && *status == StatusCode::UNAUTHORIZED
    {
        Verdict::Retry
    } else {
        verdict(attempt)
    }
}

fn sub_adapter(config: &JsonJobConfig) -> SubAdapter {
    SubAdapter {
        build_time: config.build_time.unwrap_or_default(),
        file_size: config.file_size.unwrap_or_default(),
    }
}

/// A post spends its installation's rate limit, whatever token each attempt mints.
fn credential(app: &GitHubApp, link: &QueryLinkedRepository) -> CallbackCredential {
    CallbackCredential::installation(app.api().base(), link.installation_id.into())
}

/// One line per GitHub request: where it got to, and never the token.
fn log_request(
    log: &Logger,
    claim: &Claim,
    stage: Stage,
    attempt: &CallbackAttempt,
    duration: Duration,
    pause: Option<Duration>,
) {
    slog::info!(log, "GitHub post request";
        "job" => %claim.job_uuid,
        "organization" => %claim.organization,
        "project" => %claim.project,
        "stage" => %stage,
        "attempt" => claim.attempts + 1,
        "outcome" => %attempt.class(),
        "status" => response_status(attempt).map(|status| status.as_u16()),
        "duration_ms" => millis(duration),
        "pause_ms" => pause.map(millis),
    );
}
