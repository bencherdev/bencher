use std::{sync::Arc, time::Duration};

use bencher_callback::{
    CallbackAttempt, CallbackBlock, CallbackFailure, CallbackFinish, CallbackKey, CallbackRequest,
    CallbackSender, SealedRequest, error_chain,
};
use bencher_json::{
    BENCHER_API_VERSION, CallbackContext, Clock, DateTime, JobStatus, JobUuid, JsonNewCallback,
    JsonReport, OrganizationUuid, ProjectSlug, ProjectUuid,
    runner::{CALLBACK_JOB_STATUSES, CallbackRenderError},
};
use diesel::{
    ExpressionMethods as _, OptionalExtension as _, QueryDsl as _, RunQueryDsl as _,
    SelectableHelper as _,
    r2d2::{ConnectionManager, Pool},
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
    context::{Database, DbConnection},
    model::{
        project::report::{QueryReport, ReportMode},
        runner::{CallbackOutcome, CallbackState, JobId, QueryJobCallback},
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

/// A claimed callback and the values its log lines name.
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
    #[display("headers")]
    Headers,
    /// No read connection for the report, which can change, or shutdown cut the wait short, so the
    /// callback waits for the next start.
    #[display("reader")]
    Reader,
}

/// The attempt that ended a delivery.
#[derive(Clone, Copy)]
enum LastAttempt {
    Unsent,
    /// With the receiver's status, if it answered.
    Sent(Option<StatusCode>),
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
            Self::Parse | Self::Report | Self::Headers => Some(CallbackFailure::Render),
            Self::Reader => None,
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

    pub(super) fn now(&self) -> DateTime {
        self.clock.now()
    }

    /// Deliver a callback this process has claimed, then settle it, or release it on shutdown.
    pub(super) async fn deliver(&self, log: &Logger, job_id: JobId, claimed: Instant) {
        let Some((claim, sealed, context, report)) = self.load(log, job_id, claimed).await else {
            return;
        };
        // Only the statuses the size limit counts render, and a callback fired early waits for its job.
        if !CALLBACK_JOB_STATUSES.contains(&context.job_status) {
            slog::warn!(log, "Job callback fired before its job finished"; "job" => %claim.job_uuid, "status" => %context.job_status);
            self.release(log, job_id).await;
            return;
        }
        match self.prepare(log, &claim, sealed, &context, report).await {
            Ok(request) => self.attempt(log, claim, &request).await,
            Err(unprepared) => {
                if let Some(failure) = unprepared.failure() {
                    slog::warn!(log, "Callback request not sent"; "job" => %claim.job_uuid, "stage" => %unprepared);
                    self.finish(
                        log,
                        &claim,
                        LastAttempt::Unsent,
                        CallbackFinish::Failed(failure),
                    )
                    .await;
                } else {
                    self.release(log, job_id).await;
                }
            },
        }
    }

    async fn load(
        &self,
        log: &Logger,
        job_id: JobId,
        claimed: Instant,
    ) -> Option<(Claim, Option<SealedRequest>, CallbackContext, QueryReport)> {
        let row = schema::job_callback::table
            .inner_join(
                schema::job::table
                    .inner_join(schema::report::table.inner_join(
                        schema::project::table.inner_join(schema::organization::table),
                    )),
            )
            .filter(schema::job_callback::job_id.eq(job_id))
            .filter(schema::job_callback::state.eq(CallbackState::Delivering))
            .select((
                schema::job_callback::request,
                schema::job_callback::attempts,
                schema::job::uuid,
                schema::job::status,
                QueryReport::as_select(),
                schema::project::uuid,
                schema::project::slug,
                schema::organization::uuid,
            ))
            .first::<(
                Option<SealedRequest>,
                i32,
                JobUuid,
                JobStatus,
                QueryReport,
                ProjectUuid,
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
            project_slug,
            organization,
        ) = match row {
            Ok(Some(row)) => row,
            Ok(None) => {
                slog::warn!(log, "Claimed job callback is gone"; "job_id" => ?job_id);
                return None;
            },
            Err(e) => {
                slog::error!(log, "Failed to load claimed job callback"; "job_id" => ?job_id, "error" => %e);
                self.release(log, job_id).await;
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
        sealed: Option<SealedRequest>,
        context: &CallbackContext,
        report: QueryReport,
    ) -> Result<CallbackRequest, Unprepared> {
        let Some(sealed) = sealed else {
            return Err(Unprepared::Open);
        };
        let Ok(opened) = self.key.open(claim.job_uuid, &sealed) else {
            return Err(Unprepared::Open);
        };
        // Deserializing validates the request again, so one stored before a stricter release fails here.
        let Ok(callback) = serde_json::from_slice::<JsonNewCallback>(&opened) else {
            return Err(Unprepared::Parse);
        };
        let body = match callback
            .render(context, || self.report(log, claim.job_uuid, report))
            .await
        {
            Ok(body) => body,
            Err(CallbackRenderError::Report(unprepared)) => return Err(unprepared),
            // Of everything in the body, only the report can fail to serialize.
            Err(CallbackRenderError::Json(_)) => return Err(Unprepared::Report),
        };
        let Ok(headers) = callback.delivery_headers(BENCHER_API_VERSION) else {
            return Err(Unprepared::Headers);
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
            return Err(Unprepared::Reader);
        };
        let conn = match self.reader.try_get() {
            Some(conn) => Some(Ok(conn)),
            None => {
                self.shutdown
                    .run_until_cancelled(Database::get_conn(self.reader.clone()))
                    .await
            },
        };
        let mut conn = match conn {
            Some(Ok(conn)) => conn,
            Some(Err(e)) => {
                slog::error!(log, "Failed to get a read connection for a job callback"; "job" => %job_uuid, "error" => %e);
                return Err(Unprepared::Reader);
            },
            None => return Err(Unprepared::Reader),
        };
        let Ok(report) = report.into_json(log, &mut conn, ReportMode::Full) else {
            return Err(Unprepared::Report);
        };
        Ok(report)
    }

    async fn attempt(&self, log: &Logger, mut claim: Claim, request: &CallbackRequest) {
        loop {
            let number = claim.attempts + 1;
            if number > MAX_ATTEMPTS {
                self.finish(
                    log,
                    &claim,
                    LastAttempt::Unsent,
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
                self.release(log, claim.job_id).await;
                return;
            }
            let started = Instant::now();
            let Some(attempt) = self
                .shutdown
                .run_until_cancelled(self.sender.send(request))
                .await
            else {
                self.release(log, claim.job_id).await;
                return;
            };
            // Whatever the sender did, a send error's URL never reaches the log.
            let attempt = if let CallbackAttempt::Connection(error) = attempt {
                CallbackAttempt::Connection(error.without_url())
            } else {
                attempt
            };
            log_attempt(log, &claim, request, number, &attempt, started.elapsed());
            let status = response_status(&attempt);
            let finish = match verdict(&attempt) {
                Verdict::Delivered => CallbackFinish::Delivered,
                Verdict::Retry if number < MAX_ATTEMPTS => {
                    if !self.record(log, &claim, status).await {
                        return;
                    }
                    claim.attempts = number;
                    continue;
                },
                Verdict::Retry => CallbackFinish::Failed(CallbackFailure::Exhausted),
                Verdict::Final(failure) => CallbackFinish::Failed(failure),
            };
            claim.attempts = number;
            self.finish(log, &claim, LastAttempt::Sent(status), finish)
                .await;
            return;
        }
    }

    async fn record(&self, log: &Logger, claim: &Claim, status: Option<StatusCode>) -> bool {
        let recorded = QueryJobCallback::record_attempt(
            &mut *self.connection.lock().await,
            claim.job_id,
            status,
            self.clock.now(),
        );
        match recorded {
            Ok(true) => true,
            Ok(false) => {
                slog::warn!(log, "Job callback is no longer delivering"; "job" => %claim.job_uuid);
                false
            },
            Err(e) => {
                slog::error!(log, "Failed to record a callback attempt"; "job" => %claim.job_uuid, "error" => %e);
                false
            },
        }
    }

    /// Settle the callback, recording the attempt that ended it in the same transaction.
    async fn finish(&self, log: &Logger, claim: &Claim, last: LastAttempt, finish: CallbackFinish) {
        let now = self.clock.now();
        let finished: QueryResult<bool> =
            self.connection.lock().await.immediate_transaction(|conn| {
                if let LastAttempt::Sent(status) = last {
                    QueryJobCallback::record_attempt(conn, claim.job_id, status, now)?;
                }
                QueryJobCallback::finish(conn, claim.job_id, finish.into(), now)
            });
        let latency_ms = millis(claim.claimed.elapsed());
        match (finished, finish) {
            (Ok(true), CallbackFinish::Delivered) => {
                slog::info!(log, "Callback delivered"; "job" => %claim.job_uuid, "organization" => %claim.organization, "project" => %claim.project, "attempts" => claim.attempts, "latency_ms" => latency_ms);
            },
            (Ok(true), CallbackFinish::Failed(failure)) => {
                slog::warn!(log, "Callback failed"; "job" => %claim.job_uuid, "organization" => %claim.organization, "project" => %claim.project, "attempts" => claim.attempts, "reason" => ?failure, "latency_ms" => latency_ms);
            },
            (Ok(false), _) => {
                slog::warn!(log, "Job callback is no longer delivering"; "job" => %claim.job_uuid);
            },
            (Err(e), _) => {
                slog::error!(log, "Failed to settle a job callback"; "job" => %claim.job_uuid, "error" => %e);
            },
        }
    }

    async fn release(&self, log: &Logger, job_id: JobId) {
        match QueryJobCallback::release(
            &mut *self.connection.lock().await,
            job_id,
            self.clock.now(),
        ) {
            Ok(true) => {
                slog::info!(log, "Job callback released for the next start"; "job_id" => ?job_id);
            },
            Ok(false) => {
                slog::warn!(log, "Job callback is no longer delivering"; "job_id" => ?job_id);
            },
            Err(e) => {
                slog::error!(log, "Failed to release a job callback"; "job_id" => ?job_id, "error" => %e);
            },
        }
    }
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
