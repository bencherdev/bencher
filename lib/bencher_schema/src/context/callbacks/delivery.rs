use std::{sync::Arc, time::Duration};

use bencher_callback::{
    CallbackAttempt, CallbackBlock, CallbackFailure, CallbackFinish, CallbackKey, CallbackRequest,
    CallbackSender, SealedRequest, error_chain,
};
use bencher_json::{
    BENCHER_API_VERSION, CallbackContext, Clock, JobStatus, JobUuid, JsonNewCallback, JsonReport,
    OrganizationUuid, ProjectSlug, ProjectUuid, ResourceName,
    runner::{CALLBACK_JOB_STATUSES, JobCallbackState},
};
use diesel::{
    ExpressionMethods as _, NullableExpressionMethods as _, OptionalExtension as _, QueryDsl as _,
    RunQueryDsl as _, SelectableHelper as _,
    r2d2::{ConnectionManager, Pool, PooledConnection},
    result::QueryResult,
};
use http::StatusCode;
use slog::Logger;
use tokio::{
    sync::{Mutex, Semaphore},
    time::Instant,
};
use tokio_util::sync::CancellationToken;

use crate::{
    context::DbConnection,
    model::{
        project::report::{QueryReport, ReportMode},
        runner::{CallbackOutcome, JobId, QueryJobCallback},
    },
    schema,
};

const MAX_ATTEMPTS: i32 = 3;
/// A report builds synchronously on a worker thread, so a burst of builds could take every worker.
const REPORT_BUILDS: usize = 2;

/// What every delivery task shares.
pub(super) struct Delivery {
    connection: Arc<Mutex<DbConnection>>,
    /// Builds reports, so a report of any size never holds the writer.
    reader: Pool<ConnectionManager<DbConnection>>,
    key: CallbackKey,
    sender: Arc<dyn CallbackSender>,
    shutdown: CancellationToken,
    clock: Clock,
    report_builds: Semaphore,
}

/// A pending callback and the values its log lines name.
struct Claim {
    job_id: JobId,
    job_uuid: JobUuid,
    organization: OrganizationUuid,
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
    ) -> Self {
        Self {
            connection,
            reader,
            key,
            sender,
            shutdown,
            clock,
            report_builds: Semaphore::new(REPORT_BUILDS),
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
            Ok(request) => self.attempt(log, claim, &request).await,
            Err(unprepared) => {
                if let Some(failure) = unprepared.failure() {
                    slog::warn!(log, "Callback request not sent"; "job" => %claim.job_uuid, "stage" => %unprepared);
                    self.finish(log, &claim, CallbackFinish::Failed(failure))
                        .await;
                }
            },
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
    ) -> Result<CallbackRequest, Unprepared> {
        let Ok(opened) = self.key.open(claim.job_uuid, sealed) else {
            return Err(Unprepared::Open);
        };
        // Deserializing validates the request again, so one stored before a stricter release fails here.
        let Ok(callback) = serde_json::from_slice::<JsonNewCallback>(&opened) else {
            return Err(Unprepared::Parse);
        };
        let Ok(headers) = callback.delivery_headers(BENCHER_API_VERSION) else {
            return Err(Unprepared::Parse);
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
        Ok(CallbackRequest {
            url: callback.url().clone(),
            headers,
            body,
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

    async fn attempt(&self, log: &Logger, mut claim: Claim, request: &CallbackRequest) {
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
            let started = Instant::now();
            let Some(attempt) = self
                .shutdown
                .run_until_cancelled(self.sender.send(request))
                .await
            else {
                return;
            };
            log_attempt(log, &claim, request, number, &attempt, started.elapsed());
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
}

/// Log a write to a pending callback, and whether it settled it; true when an attempt was recorded
/// and the delivery goes on.
fn settled(
    log: &Logger,
    claim: &Claim,
    written: QueryResult<bool>,
    finish: Option<CallbackFinish>,
) -> bool {
    let latency_ms = millis(claim.claimed.elapsed());
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
        CallbackAttempt::Refused(status) if retries(*status) => Verdict::Retry,
        CallbackAttempt::Refused(_) => Verdict::Final(CallbackFailure::Refused),
        CallbackAttempt::TimedOut | CallbackAttempt::Connection(_) => Verdict::Retry,
        CallbackAttempt::Blocked(block) => Verdict::Final(CallbackFailure::Blocked(block.reason())),
    }
}

/// Only these answers can change on a retry.
fn retries(status: StatusCode) -> bool {
    status == StatusCode::REQUEST_TIMEOUT
        || status == StatusCode::TOO_MANY_REQUESTS
        || status.is_server_error()
}

fn response_status(attempt: &CallbackAttempt) -> Option<StatusCode> {
    match attempt {
        CallbackAttempt::Delivered(status) | CallbackAttempt::Refused(status) => Some(*status),
        CallbackAttempt::TimedOut
        | CallbackAttempt::Connection(_)
        | CallbackAttempt::Blocked(_) => None,
    }
}

/// One line per attempt: the destination host, never the rest of the URL or a header value.
fn log_attempt(
    log: &Logger,
    claim: &Claim,
    request: &CallbackRequest,
    number: i32,
    attempt: &CallbackAttempt,
    duration: Duration,
) {
    let (address, error) = match attempt {
        CallbackAttempt::Blocked(CallbackBlock::Address { address, class: _ }) => {
            (Some(address.to_string()), None)
        },
        CallbackAttempt::Connection(error) => (None, Some(error_chain(error))),
        CallbackAttempt::Delivered(_)
        | CallbackAttempt::Refused(_)
        | CallbackAttempt::TimedOut
        | CallbackAttempt::Blocked(CallbackBlock::NotHttps) => (None, None),
    };
    slog::info!(log, "Callback attempt";
        "job" => %claim.job_uuid,
        "organization" => %claim.organization,
        "project" => %claim.project,
        "host" => request.url.host_str().unwrap_or_default(),
        "attempt" => number,
        "outcome" => %attempt.class(),
        "status" => response_status(attempt).map(|status| status.as_u16()),
        "duration_ms" => millis(duration),
        "address" => address,
        "error" => error,
    );
}

fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}
