use std::{sync::Arc, time::Duration};

#[cfg(feature = "sentry")]
use bencher_callback::CallbackHost;
use bencher_callback::{
    CallbackAttempt, CallbackBlock, CallbackCredential, CallbackFailure, CallbackFinish,
    CallbackKey, CallbackRequest, CallbackSender, RateLimit, SealedRequest, error_chain,
};
use bencher_github_client::{GitHubApiError, GitHubApp, GitHubAppError, GitHubInstallationToken};
use bencher_json::{
    BENCHER_API_VERSION, CallbackContext, Clock, JobStatus, JobUuid, JsonNewCallback, JsonReport,
    OrganizationUuid, ProjectSlug, ProjectUuid, ResourceName,
    project::github::LinkedRepositoryReason,
    runner::{CALLBACK_JOB_STATUSES, GitHubDispatch, JobCallbackState},
};
#[cfg(feature = "sentry")]
use dashmap::{DashMap, Entry};
use diesel::{
    ExpressionMethods as _, NullableExpressionMethods as _, OptionalExtension as _, QueryDsl as _,
    RunQueryDsl as _, SelectableHelper as _,
    r2d2::{ConnectionManager, Pool, PooledConnection},
    result::QueryResult,
};
use http::HeaderMap;
use http::StatusCode;
use slog::Logger;
use tokio::{
    sync::{Mutex, Semaphore},
    time::Instant,
};
use tokio_util::sync::CancellationToken;

#[cfg(feature = "sentry")]
use super::governor::Signal;
use super::governor::{Governor, rate_limit};
use crate::{
    context::DbConnection,
    model::{
        project::{
            ProjectId,
            linked_repository::QueryLinkedRepository,
            report::{QueryReport, ReportMode},
        },
        runner::{CallbackOutcome, JobId, QueryJobCallback},
    },
    schema,
};

const MAX_ATTEMPTS: i32 = 3;
/// A report builds synchronously on a worker thread, so a burst of builds could take every worker.
const REPORT_BUILDS: usize = 2;
#[cfg(feature = "sentry")]
const ALARM_INTERVAL: Duration = Duration::from_hours(1);

/// What every delivery task shares.
pub(super) struct Delivery {
    connection: Arc<Mutex<DbConnection>>,
    /// Builds reports, so a report of any size never holds the writer.
    reader: Pool<ConnectionManager<DbConnection>>,
    key: CallbackKey,
    sender: Arc<dyn CallbackSender>,
    shutdown: CancellationToken,
    clock: Clock,
    /// Mints the token of a dispatch that goes out without its own.
    github_app: Option<Arc<GitHubApp>>,
    report_builds: Semaphore,
    governor: Governor,
    #[cfg(feature = "sentry")]
    alarms: Alarms,
}

/// A pending callback and the values its log lines name.
struct Claim {
    job_id: JobId,
    job_uuid: JobUuid,
    organization: OrganizationUuid,
    project_id: ProjectId,
    project: ProjectUuid,
    attempts: i32,
    claimed: Instant,
}

/// Where preparing a request failed. Never the error itself: parsing an opened request can quote it.
#[derive(Debug, Clone, Copy, derive_more::Display)]
enum Unprepared {
    #[display("open")]
    Open,
    #[display("parse")]
    Parse,
    #[display("report")]
    Report,
    /// A dispatch without a token, on a server that has no GitHub App to mint one.
    #[display("app")]
    NoApp,
    /// A dispatch without a token, for a project with no connected linked repository.
    #[display("link")]
    NoLink,
    /// Shutdown cut the wait for a report build short, so the callback waits for the next start.
    #[display("shutdown")]
    Shutdown,
}

/// What one attempt means for the delivery.
enum Verdict {
    Delivered,
    Retry,
    Final(CallbackFailure),
}

impl From<CallbackFinish> for CallbackOutcome {
    fn from(finish: CallbackFinish) -> Self {
        match finish {
            CallbackFinish::Delivered => Self::Delivered,
            CallbackFinish::Failed(_) => Self::Failed,
        }
    }
}

impl Unprepared {
    /// The failure that ends the callback, or none when a later start can still send it.
    fn failure(self) -> Option<CallbackFailure> {
        match self {
            Self::Open => Some(CallbackFailure::Open),
            Self::Parse | Self::Report => Some(CallbackFailure::Render),
            Self::NoApp | Self::NoLink => Some(CallbackFailure::Unminted),
            Self::Shutdown => None,
        }
    }
}

impl Delivery {
    pub(super) fn new(
        connection: Arc<Mutex<DbConnection>>,
        reader: Pool<ConnectionManager<DbConnection>>,
        key: CallbackKey,
        sender: Arc<dyn CallbackSender>,
        shutdown: CancellationToken,
        clock: Clock,
        github_app: Option<Arc<GitHubApp>>,
    ) -> Self {
        Self {
            connection,
            reader,
            key,
            sender,
            shutdown,
            clock,
            github_app,
            report_builds: Semaphore::new(REPORT_BUILDS),
            governor: Governor::default(),
            #[cfg(feature = "sentry")]
            alarms: Alarms::default(),
        }
    }

    pub(super) fn connection(&self) -> &Mutex<DbConnection> {
        &self.connection
    }

    /// Deliver a pending callback of a terminal job, then settle it. Shutdown cuts a delivery
    /// short without a write, so the callback stays pending for the next start.
    pub(super) async fn deliver(&self, log: &Logger, job_id: JobId) {
        let claimed = Instant::now();
        let Some((claim, sealed, context, report)) = self.load(log, job_id, claimed).await else {
            return;
        };
        match self.prepare(log, &claim, &sealed, &context, report).await {
            Ok(outgoing) => self.attempt(log, claim, outgoing).await,
            Err(unprepared) => self.unsent(log, &claim, unprepared).await,
        }
    }

    /// Settle a callback that no attempt sent, unless a later start can still send it.
    async fn unsent(&self, log: &Logger, claim: &Claim, unprepared: Unprepared) {
        if let Some(failure) = unprepared.failure() {
            slog::warn!(log, "Callback request not sent"; "job" => %claim.job_uuid, "stage" => %unprepared);
            self.finish(log, claim, CallbackFinish::Failed(failure))
                .await;
        }
    }

    /// The job's pending callback, if it has one and the job is terminal.
    async fn load(
        &self,
        log: &Logger,
        job_id: JobId,
        claimed: Instant,
    ) -> Option<(Claim, SealedRequest, CallbackContext, QueryReport)> {
        let row = schema::job_callback::table
            .inner_join(
                schema::job::table
                    .inner_join(schema::report::table.inner_join(
                        schema::project::table.inner_join(schema::organization::table),
                    )),
            )
            .filter(schema::job_callback::job_id.eq(job_id))
            .filter(schema::job_callback::state.eq(JobCallbackState::Pending))
            .filter(schema::job::status.eq_any(CALLBACK_JOB_STATUSES))
            .select((
                // A pending callback always holds its sealed request.
                schema::job_callback::request.assume_not_null(),
                schema::job_callback::attempts,
                schema::job::uuid,
                schema::job::status,
                QueryReport::as_select(),
                schema::project::id,
                schema::project::uuid,
                schema::project::name,
                schema::project::slug,
                schema::organization::uuid,
            ))
            .first::<(
                SealedRequest,
                i32,
                JobUuid,
                JobStatus,
                QueryReport,
                ProjectId,
                ProjectUuid,
                ResourceName,
                ProjectSlug,
                OrganizationUuid,
            )>(&mut *self.connection.lock().await)
            .optional();
        let (
            sealed,
            attempts,
            job_uuid,
            job_status,
            report,
            project_id,
            project_uuid,
            project_name,
            project_slug,
            organization,
        ) = match row {
            Ok(Some(row)) => row,
            Ok(None) => return None,
            Err(e) => {
                slog::error!(log, "Failed to load a job callback"; "job_id" => ?job_id, "error" => %e);
                return None;
            },
        };
        let claim = Claim {
            job_id,
            job_uuid,
            organization,
            project_id,
            project: project_uuid,
            attempts,
            claimed,
        };
        let context = CallbackContext {
            project_uuid,
            project_name,
            project_slug,
            report_uuid: report.uuid,
            job_uuid,
            job_status,
        };
        Some((claim, sealed, context, report))
    }

    async fn prepare(
        &self,
        log: &Logger,
        claim: &Claim,
        sealed: &SealedRequest,
        context: &CallbackContext,
        report: QueryReport,
    ) -> Result<Outgoing, Unprepared> {
        let Ok(opened) = self.key.open(claim.job_uuid, sealed) else {
            return Err(Unprepared::Open);
        };
        // Deserializing validates the request again, so one stored before a stricter release fails here.
        let Ok(callback) = serde_json::from_slice::<JsonNewCallback>(&opened) else {
            return Err(Unprepared::Parse);
        };
        // A minted dispatch goes out with headers of its own, so it never builds the stored ones.
        let route = if callback.github_dispatch() == Some(GitHubDispatch::Minted) {
            let Some(app) = self.github_app.clone() else {
                return Err(Unprepared::NoApp);
            };
            Route::Minted(app)
        } else {
            let Ok(headers) = callback.delivery_headers(BENCHER_API_VERSION) else {
                return Err(Unprepared::Parse);
            };
            Route::Stored(headers)
        };
        let report = if callback.sends_report() {
            Some(self.report(log, claim.job_uuid, report).await?)
        } else {
            None
        };
        // Of everything in the body, only the report can fail to serialize.
        let Ok(body) = callback.render(context, report.as_ref()) else {
            return Err(Unprepared::Report);
        };
        Ok(match route {
            Route::Stored(headers) => Outgoing::Stored(CallbackRequest {
                url: callback.url().clone(),
                headers,
                body,
            }),
            Route::Minted(app) => Outgoing::Minted { app, body },
        })
    }

    /// The job's report, exactly as the report endpoint returns it, built on a read connection.
    async fn report(
        &self,
        log: &Logger,
        job_uuid: JobUuid,
        report: QueryReport,
    ) -> Result<JsonReport, Unprepared> {
        // Held until the build ends; shutdown stops the wait for it or for a connection.
        let Some(Ok(_build)) = self
            .shutdown
            .run_until_cancelled(self.report_builds.acquire())
            .await
        else {
            return Err(Unprepared::Shutdown);
        };
        let Some(mut conn) = self
            .shutdown
            .run_until_cancelled(self.read_connection(log, job_uuid))
            .await
        else {
            return Err(Unprepared::Shutdown);
        };
        let Ok(report) = report.into_json(log, &mut conn, ReportMode::Full) else {
            return Err(Unprepared::Report);
        };
        Ok(report)
    }

    /// A read connection, however long the pool stays busy: giving up would leave the callback
    /// waiting for the next start, and a thread blocked on the pool would hold up shutdown.
    async fn read_connection(
        &self,
        log: &Logger,
        job_uuid: JobUuid,
    ) -> PooledConnection<ConnectionManager<DbConnection>> {
        let mut failed = false;
        loop {
            if let Some(conn) = self.reader.try_get() {
                if failed {
                    slog::info!(log, "Got a read connection for a job callback"; "job" => %job_uuid);
                }
                return conn;
            }
            if !failed {
                slog::warn!(log, "No read connection for a job callback, retrying"; "job" => %job_uuid);
                failed = true;
            }
            tokio::time::sleep(self.reader.connection_timeout()).await;
        }
    }

    async fn attempt(&self, log: &Logger, mut claim: Claim, outgoing: Outgoing) {
        loop {
            let number = claim.attempts + 1;
            // A count at the limit, as a restart can find, never sends again.
            if number > MAX_ATTEMPTS {
                self.finish(
                    log,
                    &claim,
                    CallbackFinish::Failed(CallbackFailure::Exhausted),
                )
                .await;
                return;
            }
            if let Some(wait) = wait_before(number)
                && self
                    .shutdown
                    .run_until_cancelled(tokio::time::sleep(wait))
                    .await
                    .is_none()
            {
                return;
            }
            // A dispatch without a token goes to the project's link as it stands at each attempt.
            let target = match &outgoing {
                Outgoing::Stored(request) => Target::Stored(request),
                Outgoing::Minted { app, body } => {
                    let Some(link) = self.connected_link(log, &claim).await else {
                        self.unsent(log, &claim, Unprepared::NoLink).await;
                        return;
                    };
                    Target::Minted { app, link, body }
                },
            };
            let credential = target.credential();
            let host = target.host();
            let Some(Some(turn)) = self
                .shutdown
                .run_until_cancelled(self.governor.turn(&credential))
                .await
            else {
                return;
            };
            let started = Instant::now();
            let Some((attempt, stage)) = self
                .shutdown
                .run_until_cancelled(self.send(log, &claim, target))
                .await
            else {
                return;
            };
            let pause = turn.settle(&attempt, self.clock.timestamp());
            log_attempt(
                log,
                &claim,
                Reached { host: &host, stage },
                number,
                &attempt,
                started.elapsed(),
                pause,
            );
            #[cfg(feature = "sentry")]
            self.alarm(&claim, &credential, &attempt);
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::CallbackAttempt(
                attempt.class(),
            ));
            claim.attempts = number;
            let finish = match verdict(&attempt) {
                Verdict::Delivered => Some(CallbackFinish::Delivered),
                Verdict::Retry if number < MAX_ATTEMPTS => None,
                Verdict::Retry => Some(CallbackFinish::Failed(CallbackFailure::Exhausted)),
                Verdict::Final(failure) => Some(CallbackFinish::Failed(failure)),
            };
            let recorded = QueryJobCallback::record_attempt(
                &mut *self.connection.lock().await,
                claim.job_id,
                response_status(&attempt),
                finish.map(Into::into),
                self.clock.now(),
            );
            if !settled(log, &claim, recorded, finish) {
                return;
            }
        }
    }

    /// The project's connected link as it stands now, without which a dispatch with no token of
    /// its own never goes out.
    async fn connected_link(&self, log: &Logger, claim: &Claim) -> Option<QueryLinkedRepository> {
        let link = QueryLinkedRepository::get_connected(
            &mut *self.connection.lock().await,
            claim.project_id,
        );
        link.unwrap_or_else(|e| {
            slog::error!(log, "Failed to read a linked repository"; "job" => %claim.job_uuid, "error" => %e);
            None
        })
    }

    async fn send(
        &self,
        log: &Logger,
        claim: &Claim,
        target: Target<'_>,
    ) -> (CallbackAttempt, Stage) {
        match target {
            Target::Stored(request) => (self.sender.send(request).await, Stage::Send),
            Target::Minted { app, link, body } => self.dispatch(log, claim, app, link, body).await,
        }
    }

    /// Mint a token for the linked repository, send the dispatch to that repository with it, and
    /// keep the repository's current full name. The token is dropped with the request, sent or not.
    async fn dispatch(
        &self,
        log: &Logger,
        claim: &Claim,
        app: &GitHubApp,
        link: QueryLinkedRepository,
        body: &str,
    ) -> (CallbackAttempt, Stage) {
        let (link, GitHubInstallationToken { token, full_name }) =
            match self.mint(log, claim, app, link).await {
                Ok(minted) => minted,
                Err(unminted) => return unminted,
            };
        let Ok(headers) = GitHubDispatch::minted_headers(BENCHER_API_VERSION, &token) else {
            return (CallbackAttempt::Unminted, Stage::Mint);
        };
        drop(token);
        let request = CallbackRequest {
            url: app.dispatch_url(&full_name),
            headers,
            body: body.to_owned(),
        };
        let attempt = self.sender.send(&request).await;
        drop(request);
        let written = link.write_full_name(&mut *self.connection.lock().await, &full_name);
        if let Err(e) = written {
            slog::error!(log, "Failed to write a linked repository's full name"; "job" => %claim.job_uuid, "error" => %e);
        }
        (attempt, Stage::Send)
    }

    /// Mint a token for the link. A refusal that says the repository left the installation looks
    /// up the installation that holds it now: the link moves there and mints once more, or with
    /// none, disconnects. A lookup that fails leaves the link as it is.
    async fn mint(
        &self,
        log: &Logger,
        claim: &Claim,
        app: &GitHubApp,
        link: QueryLinkedRepository,
    ) -> Result<(QueryLinkedRepository, GitHubInstallationToken), (CallbackAttempt, Stage)> {
        let now = self.clock.now();
        let refusal = match app
            .mint(now, link.installation_id, link.repository_id)
            .await
        {
            Ok(token) => return Ok((link, token)),
            Err(refusal) => refusal,
        };
        let Some(reason) = gone(&refusal) else {
            return Err((refused(refusal), Stage::Mint));
        };
        let installation = match app.repository_installation(now, &link.full_name).await {
            Ok(Some(installation)) => installation,
            Ok(None) => {
                self.disconnect(log, claim, &link, reason).await;
                return Err((refused(refusal), Stage::Mint));
            },
            Err(lookup) => return Err((refused(lookup), Stage::Lookup)),
        };
        let moved = link.move_to(&mut *self.connection.lock().await, installation);
        let link = match moved {
            Ok(Some(moved)) => {
                slog::info!(log, "Linked repository moved"; "job" => %claim.job_uuid, "project" => %claim.project, "installation" => %installation);
                moved
            },
            // Replaced since, so it is no longer this link to mint for.
            Ok(None) => return Err((refused(refusal), Stage::Mint)),
            Err(e) => {
                slog::error!(log, "Failed to move a linked repository"; "job" => %claim.job_uuid, "error" => %e);
                return Err((refused(refusal), Stage::Mint));
            },
        };
        match app
            .mint(now, link.installation_id, link.repository_id)
            .await
        {
            Ok(token) => Ok((link, token)),
            Err(refusal) => {
                if let Some(reason) = gone(&refusal) {
                    self.disconnect(log, claim, &link, reason).await;
                }
                Err((refused(refusal), Stage::Mint))
            },
        }
    }

    async fn disconnect(
        &self,
        log: &Logger,
        claim: &Claim,
        link: &QueryLinkedRepository,
        reason: LinkedRepositoryReason,
    ) {
        let disconnected = link.disconnect(&mut *self.connection.lock().await, reason);
        match disconnected {
            Ok(true) => {
                slog::warn!(log, "Linked repository disconnected"; "job" => %claim.job_uuid, "project" => %claim.project, "reason" => ?reason);
            },
            Ok(false) => {},
            Err(e) => {
                slog::error!(log, "Failed to disconnect a linked repository"; "job" => %claim.job_uuid, "error" => %e);
            },
        }
    }

    /// Settle the callback without an attempt.
    async fn finish(&self, log: &Logger, claim: &Claim, finish: CallbackFinish) {
        let finished = QueryJobCallback::finish(
            &mut *self.connection.lock().await,
            claim.job_id,
            finish.into(),
            self.clock.now(),
        );
        settled(log, claim, finished, Some(finish));
    }

    /// Tell Sentry of a rate limit, once an hour per host at most.
    #[cfg(feature = "sentry")]
    fn alarm(&self, claim: &Claim, credential: &CallbackCredential, attempt: &CallbackAttempt) {
        if let Some((_, signal)) = rate_limit(attempt)
            && let Some(status) = response_status(attempt)
            && self.alarms.due(credential.host())
        {
            sentry::capture_error(&RateLimited {
                host: credential.host().clone(),
                status,
                signal,
                job: claim.job_uuid,
                organization: claim.organization,
                project: claim.project,
            });
        }
    }
}

/// A callback ready for its attempts.
enum Outgoing {
    /// Sent exactly as stored.
    Stored(CallbackRequest),
    /// A dispatch with no token of its own, which goes out only with a token minted for the
    /// project's linked repository, to that repository.
    Minted { app: Arc<GitHubApp>, body: String },
}

/// How a callback goes out: with its stored headers, or with a token the app mints.
enum Route {
    Stored(HeaderMap),
    Minted(Arc<GitHubApp>),
}

/// Where one attempt goes.
enum Target<'a> {
    Stored(&'a CallbackRequest),
    Minted {
        app: &'a GitHubApp,
        link: QueryLinkedRepository,
        body: &'a str,
    },
}

/// Where an attempt went, and how far it got.
struct Reached<'a> {
    host: &'a str,
    stage: Stage,
}

/// What an attempt reached: the receiver, or the mint or the lookup before it.
#[derive(Debug, Clone, Copy, derive_more::Display)]
enum Stage {
    #[display("send")]
    Send,
    #[display("mint")]
    Mint,
    #[display("lookup")]
    Lookup,
}

impl Target<'_> {
    /// A minted dispatch spends its installation's rate limit, whatever token each attempt mints.
    fn credential(&self) -> CallbackCredential {
        match self {
            Self::Stored(request) => CallbackCredential::of(request),
            Self::Minted { app, link, body: _ } => CallbackCredential::installation(
                &app.dispatch_url(&link.full_name),
                link.installation_id.into(),
            ),
        }
    }

    fn host(&self) -> String {
        let url = match self {
            Self::Stored(request) => request.url.clone(),
            Self::Minted { app, link, body: _ } => app.dispatch_url(&link.full_name),
        };
        url.host_str().unwrap_or_default().to_owned()
    }
}

/// Why a refused mint says the repository left the installation, if it says so.
fn gone(refusal: &GitHubAppError) -> Option<LinkedRepositoryReason> {
    if let GitHubAppError::Mint(GitHubApiError::NotFound) = refusal {
        Some(LinkedRepositoryReason::Uninstalled)
    } else if let GitHubAppError::Mint(GitHubApiError::RepositoryInaccessible) = refusal {
        Some(LinkedRepositoryReason::Removed)
    } else {
        None
    }
}

/// A failed mint or lookup as an attempt: GitHub's answer, as a refused send's would be, with its
/// rate limit. No refusal holds the token, and a request error goes on without its URL.
fn refused(failure: GitHubAppError) -> CallbackAttempt {
    let (GitHubAppError::Mint(failure) | GitHubAppError::Installation(failure)) = failure else {
        return CallbackAttempt::Unminted;
    };
    match failure {
        GitHubApiError::NotFound => {
            CallbackAttempt::Refused(StatusCode::NOT_FOUND, RateLimit::default())
        },
        GitHubApiError::RepositoryInaccessible | GitHubApiError::PermissionsNotGranted => {
            CallbackAttempt::Refused(StatusCode::UNPROCESSABLE_ENTITY, RateLimit::default())
        },
        GitHubApiError::Status(status, headers) => {
            CallbackAttempt::Refused(status, RateLimit::read(status, &headers))
        },
        GitHubApiError::Request(error) => {
            if error.is_timeout() {
                CallbackAttempt::TimedOut
            } else {
                CallbackAttempt::Connection(Arc::new(error.without_url()))
            }
        },
        GitHubApiError::Json(_) | GitHubApiError::RepositoryName(_) => CallbackAttempt::Unminted,
    }
}

/// A receiver's rate limit, named by its host and never by its URL path, a header, or a body.
#[cfg(feature = "sentry")]
#[derive(Debug, thiserror::Error)]
#[error(
    "Callback rate limited by {host}: {status} ({signal}) for job {job}, organization {organization}, project {project}"
)]
struct RateLimited {
    host: CallbackHost,
    status: StatusCode,
    signal: Signal,
    job: JobUuid,
    organization: OrganizationUuid,
    project: ProjectUuid,
}

/// When Sentry last heard of each host's rate limit, within the hour.
#[cfg(feature = "sentry")]
#[derive(Default)]
struct Alarms(DashMap<CallbackHost, Instant>);

#[cfg(feature = "sentry")]
impl Alarms {
    /// Whether Sentry has not heard of the host within the hour, marking it heard if so.
    fn due(&self, host: &CallbackHost) -> bool {
        self.0.retain(|_, at| at.elapsed() < ALARM_INTERVAL);
        match self.0.entry(host.clone()) {
            Entry::Occupied(_) => false,
            Entry::Vacant(entry) => {
                entry.insert(Instant::now());
                true
            },
        }
    }
}

/// Log a write to a pending callback, and whether it settled it; true when an attempt was recorded
/// and the delivery goes on.
fn settled(
    log: &Logger,
    claim: &Claim,
    written: QueryResult<bool>,
    finish: Option<CallbackFinish>,
) -> bool {
    let latency = claim.claimed.elapsed();
    // Only a settle that committed ended the delivery.
    #[cfg(feature = "otel")]
    if let (Ok(true), Some(finish)) = (&written, finish) {
        bencher_otel::ApiMeter::record(
            bencher_otel::ApiHistogram::CallbackFinishDuration(finish),
            latency.as_secs_f64(),
        );
        bencher_otel::ApiMeter::record(
            bencher_otel::ApiHistogram::CallbackFinishAttempts(finish),
            f64::from(claim.attempts),
        );
    }
    let latency_ms = millis(latency);
    match (written, finish) {
        (Ok(true), None) => return true,
        (Ok(true), Some(CallbackFinish::Delivered)) => {
            slog::info!(log, "Callback delivered"; "job" => %claim.job_uuid, "organization" => %claim.organization, "project" => %claim.project, "attempts" => claim.attempts, "latency_ms" => latency_ms);
        },
        (Ok(true), Some(CallbackFinish::Failed(failure))) => {
            slog::warn!(log, "Callback failed"; "job" => %claim.job_uuid, "organization" => %claim.organization, "project" => %claim.project, "attempts" => claim.attempts, "reason" => ?failure, "latency_ms" => latency_ms);
        },
        (Ok(false), _) => {
            slog::warn!(log, "Job callback is no longer pending"; "job" => %claim.job_uuid);
        },
        (Err(e), _) => {
            slog::error!(log, "Failed to write a job callback"; "job" => %claim.job_uuid, "error" => %e);
        },
    }
    false
}

/// The wait before an attempt, by its number alone, however the delivery reached it.
fn wait_before(attempt: i32) -> Option<Duration> {
    match attempt {
        2 => Some(Duration::from_secs(4)),
        3 => Some(Duration::from_secs(16)),
        _ => None,
    }
}

fn verdict(attempt: &CallbackAttempt) -> Verdict {
    match attempt {
        CallbackAttempt::Delivered(_) => Verdict::Delivered,
        CallbackAttempt::Refused(status, _)
            if retries(*status) || rate_limit(attempt).is_some() =>
        {
            Verdict::Retry
        },
        CallbackAttempt::Refused(..) => Verdict::Final(CallbackFailure::Refused),
        CallbackAttempt::TimedOut | CallbackAttempt::Connection(_) => Verdict::Retry,
        CallbackAttempt::Blocked(block) => Verdict::Final(CallbackFailure::Blocked(block.reason())),
        CallbackAttempt::Unminted => Verdict::Final(CallbackFailure::Unminted),
    }
}

/// Besides a rate limit, only these answers can change on a retry.
fn retries(status: StatusCode) -> bool {
    status == StatusCode::REQUEST_TIMEOUT || status.is_server_error()
}

fn response_status(attempt: &CallbackAttempt) -> Option<StatusCode> {
    match attempt {
        CallbackAttempt::Delivered(status) | CallbackAttempt::Refused(status, _) => Some(*status),
        CallbackAttempt::TimedOut
        | CallbackAttempt::Connection(_)
        | CallbackAttempt::Blocked(_)
        | CallbackAttempt::Unminted => None,
    }
}

/// One line per attempt: the destination host, never the rest of the URL or a header value.
fn log_attempt(
    log: &Logger,
    claim: &Claim,
    Reached { host, stage }: Reached<'_>,
    number: i32,
    attempt: &CallbackAttempt,
    duration: Duration,
    pause: Option<Duration>,
) {
    let (address, error) = match attempt {
        CallbackAttempt::Blocked(CallbackBlock::Address { address, class: _ }) => {
            (Some(address.to_string()), None)
        },
        CallbackAttempt::Connection(error) => (None, Some(error_chain(error))),
        CallbackAttempt::Delivered(_)
        | CallbackAttempt::Refused(..)
        | CallbackAttempt::TimedOut
        | CallbackAttempt::Blocked(CallbackBlock::NotHttps)
        | CallbackAttempt::Unminted => (None, None),
    };
    slog::info!(log, "Callback attempt";
        "job" => %claim.job_uuid,
        "organization" => %claim.organization,
        "project" => %claim.project,
        "host" => host,
        "stage" => %stage,
        "attempt" => number,
        "outcome" => %attempt.class(),
        "status" => response_status(attempt).map(|status| status.as_u16()),
        "duration_ms" => millis(duration),
        "pause_ms" => pause.map(millis),
        "address" => address,
        "error" => error,
    );
}

fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}
