//! Persistent WebSocket channel for runner job lifecycle.
//!
//! Single WebSocket connection that handles job assignment, execution status,
//! and stays open between jobs.

use std::time::Duration;

use bencher_json::{
    DEFAULT_POLL_TIMEOUT, JobPriority, JobStatus, JsonClaimedJob, JsonSpec, RunnerResourceId,
    runner::{CloseReason, JsonIterationOutput, RunnerMessage, ServerMessage},
};
use bencher_oci_storage::OciStorageError;
use bencher_schema::{
    auth_conn,
    context::ApiContext,
    error::{resource_conflict_err, resource_not_found_err},
    model::{
        runner::{JobId, QueryJob, UpdateJob, job::spawn_heartbeat_timeout},
        spec::QuerySpec,
    },
    schema, write_conn,
};
use bencher_token::OciAction;
use diesel::{
    BoolExpressionMethods as _, ExpressionMethods as _, OptionalExtension as _, QueryDsl as _,
    RunQueryDsl as _, dsl::exists, dsl::not,
};
use dropshot::{
    HttpError, Path, RequestContext, WebsocketChannelResult, WebsocketConnection, channel,
};
use futures::{SinkExt as _, StreamExt as _};
use schemars::JsonSchema;
use serde::Deserialize;
use tokio_tungstenite::tungstenite::{
    Message,
    protocol::{Role, WebSocketConfig},
};

use crate::runner_token::RunnerToken;

// --- WebSocket message handlers ---

/// Compute the maximum allowed elapsed seconds for a job: timeout + grace period.
fn job_timeout_limit(timeout: bencher_json::Timeout, grace: Duration) -> i64 {
    #[expect(
        clippy::cast_possible_wrap,
        reason = "timeout max i32::MAX + grace period fits in i64"
    )]
    let limit = u64::from(u32::from(timeout)) as i64 + grace.as_secs() as i64;
    limit
}

/// Errors from WebSocket channel operations during runner job execution.
#[derive(Debug, thiserror::Error)]
enum ChannelError {
    #[error("{0}")]
    Http(#[from] HttpError),

    #[error("{0}")]
    Diesel(#[from] diesel::result::Error),

    #[error("{0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),

    #[error("{0}")]
    Json(#[from] serde_json::Error),

    /// Job is in an unexpected state for the requested transition.
    #[error("Invalid state transition to {target:?} for job {job_id:?}, found {current:?}")]
    InvalidStateTransition {
        job_id: JobId,
        target: JobStatus,
        current: JobStatus,
    },
}

/// Handle a heartbeat timeout by reading the job and deciding the right status.
///
/// If the job has exceeded its configured timeout + grace period, it is marked `Canceled`
/// (ran too long). Otherwise it is marked `Failed` (lost contact with runner).
/// Returns the [`CloseReason`] for the WebSocket close frame.
async fn handle_timeout(
    log: &slog::Logger,
    context: &ApiContext,
    job_id: JobId,
) -> Result<CloseReason, ChannelError> {
    let job: QueryJob = schema::job::table
        .filter(schema::job::id.eq(job_id))
        .first(auth_conn!(context))?;

    if job.status.has_run() {
        slog::info!(log, "Heartbeat timeout: job already in terminal state"; "job_id" => ?job_id);
        return Ok(CloseReason::HeartbeatTimeout);
    }

    let now = context.clock.now();

    let (status, reason) = if let Some(started) = job.started {
        let elapsed = (now.timestamp() - started.timestamp()).max(0);
        let limit = job_timeout_limit(job.timeout, context.job_timeout_grace_period);
        if elapsed > limit {
            (JobStatus::Canceled, CloseReason::JobTimeoutExceeded)
        } else {
            (JobStatus::Failed, CloseReason::HeartbeatTimeout)
        }
    } else {
        (JobStatus::Failed, CloseReason::HeartbeatTimeout)
    };

    slog::warn!(log, "Marking job"; "job_id" => ?job_id, "status" => ?status, "reason" => ?reason);
    let update = UpdateJob::terminate(status, now);
    let updated = update.execute_if_either_status(
        write_conn!(context),
        job_id,
        JobStatus::Claimed,
        JobStatus::Running,
    )?;
    if updated == 0 {
        slog::info!(log, "Timeout: job already in terminal state"; "job_id" => ?job_id);
    }
    Ok(reason)
}

/// Handle a message from the runner and return the appropriate response
/// along with an optional [`CloseReason`] for terminal messages.
async fn handle_runner_message(
    log: &slog::Logger,
    context: &ApiContext,
    job: &QueryJob,
    msg: RunnerMessage,
) -> Result<(ServerMessage, Option<CloseReason>), ChannelError> {
    match msg {
        RunnerMessage::Ready { .. } => {
            slog::warn!(log, "Unexpected Ready message during job execution"; "job_id" => ?job.id);
            // Ack is the only safe response — Cancel would terminate the job.
            // The heartbeat timer is NOT reset for Ready (handled by caller).
        },
        RunnerMessage::Running => {
            slog::info!(log, "Job running"; "job_id" => ?job.id);
            if let Some(cancel) = handle_running(log, context, job.id).await? {
                return Ok((cancel, Some(CloseReason::JobCanceled)));
            }
        },
        RunnerMessage::Heartbeat => {
            slog::debug!(log, "Job heartbeat"; "job_id" => ?job.id);
            if let Some(cancel) = handle_heartbeat(log, context, job.id).await? {
                return Ok((cancel, Some(CloseReason::JobCanceled)));
            }
        },
        RunnerMessage::Completed { results } => {
            slog::info!(log, "Job completed"; "job_id" => ?job.id, "iterations" => results.len());
            handle_completed(log, context, job, results).await?;
            return Ok((ServerMessage::Ack, Some(CloseReason::JobCompleted)));
        },
        RunnerMessage::Failed { results, error } => {
            slog::warn!(log, "Job failed"; "job_id" => ?job.id, "error" => &error);
            handle_failed(log, context, job, results, error).await?;
            return Ok((ServerMessage::Ack, Some(CloseReason::JobFailed)));
        },
        RunnerMessage::Canceled => {
            slog::info!(log, "Job cancellation acknowledged"; "job_id" => ?job.id);
            handle_canceled(log, context, job.id).await?;
            return Ok((ServerMessage::Ack, Some(CloseReason::JobCanceledByRunner)));
        },
    }

    Ok((ServerMessage::Ack, None))
}

/// Handle a Running message: transition job from Claimed to Running,
/// or update heartbeat if already Running (reconnection case).
///
/// Uses conditional UPDATEs with status filters to avoid TOCTOU races.
async fn handle_running(
    log: &slog::Logger,
    context: &ApiContext,
    job_id: JobId,
) -> Result<Option<ServerMessage>, ChannelError> {
    let now = context.clock.now();

    // Try reconnection case first: already Running, just update heartbeat
    let reconnect_update = UpdateJob::heartbeat(now);
    let updated =
        reconnect_update.execute_if_status(write_conn!(context), job_id, JobStatus::Running)?;

    if updated > 0 {
        slog::info!(log, "Runner reconnected to running job"; "job_id" => ?job_id);
        return Ok(None);
    }

    // Try normal transition: Claimed -> Running
    let transition_update = UpdateJob::start(now);
    let updated =
        transition_update.execute_if_status(write_conn!(context), job_id, JobStatus::Claimed)?;

    if updated > 0 {
        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerJobUpdate(
            bencher_otel::JobStatusKind::Running,
        ));
        return Ok(None);
    }

    // Neither matched — check if job was canceled
    let current_job: QueryJob = schema::job::table
        .filter(schema::job::id.eq(job_id))
        .first(auth_conn!(context))?;
    if current_job.status == JobStatus::Canceled {
        slog::info!(log, "Job was canceled before Running transition"; "job_id" => ?job_id);
        return Ok(Some(ServerMessage::Cancel));
    }
    slog::warn!(log, "Unexpected job state during Running transition"; "job_id" => ?job_id, "status" => ?current_job.status);
    Ok(None)
}

/// Handle a Heartbeat message: update `last_heartbeat` and check for cancellation and timeout.
async fn handle_heartbeat(
    log: &slog::Logger,
    context: &ApiContext,
    job_id: JobId,
) -> Result<Option<ServerMessage>, ChannelError> {
    let now = context.clock.now();

    let job: QueryJob = schema::job::table
        .filter(schema::job::id.eq(job_id))
        .first(auth_conn!(context))
        .map_err(resource_not_found_err!(Job, job_id))?;

    // Check if job was canceled
    if job.status == JobStatus::Canceled {
        return Ok(Some(ServerMessage::Cancel));
    }

    // Check if job has exceeded its timeout
    if let Some(started) = job.started {
        let elapsed = (now.timestamp() - started.timestamp()).max(0);
        let limit = job_timeout_limit(job.timeout, context.job_timeout_grace_period);
        if elapsed > limit {
            slog::warn!(log, "Job timeout exceeded during heartbeat"; "job_id" => ?job_id, "elapsed" => elapsed, "limit" => limit);
            let cancel_update = UpdateJob::terminate(JobStatus::Canceled, now);
            let updated = cancel_update.execute_if_either_status(
                write_conn!(context),
                job_id,
                JobStatus::Claimed,
                JobStatus::Running,
            )?;
            if updated > 0 {
                #[cfg(feature = "otel")]
                bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerJobUpdate(
                    bencher_otel::JobStatusKind::Canceled,
                ));
            }
            return Ok(Some(ServerMessage::Cancel));
        }
    }

    let update = UpdateJob::heartbeat(now);

    // It is okay to wait until here to get the write lock
    // Worst case, we add an extra write if the job was canceled between reads
    // Use status filter to avoid overwriting a concurrent cancellation
    update.execute_if_either_status(
        write_conn!(context),
        job_id,
        JobStatus::Claimed,
        JobStatus::Running,
    )?;

    // TODO: Billing logic - check elapsed minutes and bill to Stripe

    Ok(None)
}

/// Handle a Completed message: transition job from Running to Completed,
/// store output, and process benchmark results into the report.
///
/// Uses a status filter on the UPDATE to avoid TOCTOU races.
async fn handle_completed(
    log: &slog::Logger,
    context: &ApiContext,
    job: &QueryJob,
    results: Vec<JsonIterationOutput>,
) -> Result<(), ChannelError> {
    let now = context.clock.now();

    let update = UpdateJob::terminate(JobStatus::Completed, now);

    let updated = update.execute_if_status(write_conn!(context), job.id, JobStatus::Running)?;

    if updated == 0 {
        // Re-read the job to determine what happened
        let current_job: QueryJob = schema::job::table
            .filter(schema::job::id.eq(job.id))
            .first(auth_conn!(context))
            .map_err(resource_not_found_err!(Job, job.id))?;

        if matches!(
            current_job.status,
            JobStatus::Completed | JobStatus::Processed
        ) {
            slog::debug!(log, "Job already completed (idempotent duplicate)"; "job_id" => ?job.id);
            return Ok(());
        }
        if current_job.status.has_run() {
            slog::warn!(log, "Job already in terminal state, completion report lost"; "job_id" => ?job.id, "current_status" => ?current_job.status);
            return Ok(());
        }
        return Err(ChannelError::InvalidStateTransition {
            job_id: job.id,
            target: JobStatus::Completed,
            current: current_job.status,
        });
    }

    #[cfg(feature = "otel")]
    {
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerJobUpdate(
            bencher_otel::JobStatusKind::Completed,
        ));

        let duration_secs = job.created.elapsed_secs(now);
        let tier = bencher_otel::PriorityTier::from_priority(job.priority.into());
        bencher_otel::ApiMeter::record(
            bencher_otel::ApiHistogram::JobCompleteDuration(tier),
            duration_secs,
        );
    }

    // Store output in blob storage (best-effort)
    let job_output = bencher_json::runner::JsonJobOutput {
        results,
        error: None,
    };
    if let Err(e) = store_job_output(context, job, &job_output).await {
        slog::error!(log, "Failed to store job output"; "job_id" => ?job.id, "error" => %e);
    }

    let bencher_json::runner::JsonJobOutput { results, error: _ } = job_output;

    // Process benchmark results into the report
    if let Err(e) = job.process_results(log, context, results, now).await {
        slog::error!(log, "Failed to process job results"; "job_id" => ?job.id, "error" => %e);
        // Transition to Failed so startup recovery doesn't retry forever
        let failed_update = UpdateJob::set_status(JobStatus::Failed, now);
        failed_update.execute_if_status(write_conn!(context), job.id, JobStatus::Completed)?;
        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerJobUpdate(
            bencher_otel::JobStatusKind::Failed,
        ));
        return Ok(());
    }

    // Transition to Processed on success
    let processed_update = UpdateJob::set_status(JobStatus::Processed, now);
    let updated =
        processed_update.execute_if_status(write_conn!(context), job.id, JobStatus::Completed)?;
    if updated == 0 {
        slog::info!(log, "Job already changed state during Processed transition"; "job_id" => ?job.id);
    }

    #[cfg(feature = "otel")]
    if updated > 0 {
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerJobUpdate(
            bencher_otel::JobStatusKind::Processed,
        ));
    }

    Ok(())
}

/// Handle a Failed message: transition job from Claimed or Running to Failed.
///
/// Uses a status filter on the UPDATE to avoid TOCTOU races.
async fn handle_failed(
    log: &slog::Logger,
    context: &ApiContext,
    job: &QueryJob,
    results: Vec<JsonIterationOutput>,
    error: String,
) -> Result<(), ChannelError> {
    let now = context.clock.now();

    let update = UpdateJob::terminate(JobStatus::Failed, now);

    let updated = update.execute_if_either_status(
        write_conn!(context),
        job.id,
        JobStatus::Claimed,
        JobStatus::Running,
    )?;

    if updated == 0 {
        // Re-read the job to determine what happened
        let current_job: QueryJob = schema::job::table
            .filter(schema::job::id.eq(job.id))
            .first(auth_conn!(context))
            .map_err(resource_not_found_err!(Job, job.id))?;

        if current_job.status == JobStatus::Failed {
            slog::debug!(log, "Job already failed (idempotent duplicate)"; "job_id" => ?job.id);
            return Ok(());
        }
        if current_job.status.has_run() {
            slog::warn!(log, "Job already in terminal state, failure report lost"; "job_id" => ?job.id, "current_status" => ?current_job.status);
            return Ok(());
        }
        return Err(ChannelError::InvalidStateTransition {
            job_id: job.id,
            target: JobStatus::Failed,
            current: current_job.status,
        });
    }

    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerJobUpdate(
        bencher_otel::JobStatusKind::Failed,
    ));

    // Store output in blob storage (best-effort)
    let job_output = bencher_json::runner::JsonJobOutput {
        results,
        error: Some(error),
    };
    if let Err(e) = store_job_output(context, job, &job_output).await {
        slog::error!(log, "Failed to store job output"; "job_id" => ?job.id, "error" => %e);
    }

    Ok(())
}

/// Store job output in blob storage.
async fn store_job_output(
    context: &ApiContext,
    job: &QueryJob,
    job_output: &bencher_json::runner::JsonJobOutput,
) -> Result<(), OciStorageError> {
    let project_uuid = job.config.project;
    context
        .oci_storage()
        .job_output()
        .put(project_uuid, job.uuid, job_output)
        .await
}

/// Handle a Canceled message: runner acknowledges cancellation, ensure job is in Canceled state.
///
/// Uses a status filter on the UPDATE to avoid TOCTOU races. If zero rows updated,
/// re-reads to check whether the job is already Canceled (idempotent success).
async fn handle_canceled(
    log: &slog::Logger,
    context: &ApiContext,
    job_id: JobId,
) -> Result<(), ChannelError> {
    let now = context.clock.now();

    // Try to transition from Claimed or Running to Canceled
    let update = UpdateJob::terminate(JobStatus::Canceled, now);

    let updated = update.execute_if_either_status(
        write_conn!(context),
        job_id,
        JobStatus::Claimed,
        JobStatus::Running,
    )?;

    if updated > 0 {
        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerJobUpdate(
            bencher_otel::JobStatusKind::Canceled,
        ));
        return Ok(());
    }

    // Zero rows updated — check if already Canceled (idempotent) or invalid state
    let job: QueryJob = schema::job::table
        .filter(schema::job::id.eq(job_id))
        .first(auth_conn!(context))
        .map_err(resource_not_found_err!(Job, job_id))?;

    if job.status == JobStatus::Canceled {
        slog::debug!(log, "Job already canceled"; "job_id" => ?job_id);
        return Ok(());
    }

    slog::warn!(log, "Invalid state transition to Canceled (concurrent state change)"; "job_id" => ?job_id, "current_status" => ?job.status);
    Err(ChannelError::InvalidStateTransition {
        job_id,
        target: JobStatus::Canceled,
        current: job.status,
    })
}

// --- Job claiming ---

// Table aliases for correlated subqueries in the claim query.
// job_org is used to check if an org already has a running job (Free tier limit).
// job_ip is used to check if a source IP already has a running job (Unclaimed tier limit).
diesel::alias!(schema::job as job_org: JobOrg);
diesel::alias!(schema::job as job_ip: JobIp);

/// Priority threshold for Enterprise/Team tiers (unlimited concurrency)
const PRIORITY_UNLIMITED: JobPriority = JobPriority::Team;
/// Priority threshold for Free tier (1 per org concurrency)
const PRIORITY_FREE: JobPriority = JobPriority::Free;

/// Poll interval for long-polling (1 second)
const POLL_INTERVAL: Duration = Duration::from_secs(1);

/// Attempt to claim a pending job with tier-based concurrency limits.
///
/// Priority tiers:
/// - Enterprise / Team: Unlimited concurrent jobs
/// - Free: 1 concurrent job per organization
/// - Unclaimed: 1 concurrent job per source IP
///
/// Returns `Ok(Some(job))` if a job was claimed, `Ok(None)` if no eligible jobs available.
/// OCI runner token TTL: 10 minutes (enough for image pull, short enough to limit exposure)
const OCI_RUNNER_TOKEN_TTL: u32 = 600;

async fn try_claim_job(
    context: &ApiContext,
    runner_token: &RunnerToken,
) -> Result<Option<JsonClaimedJob>, HttpError> {
    use schema::job::dsl::{created, id, organization_id, priority, source_ip, status};

    // Tier 1: Enterprise/Team (priority >= 200) - no concurrency limit
    let tier_unlimited = priority.ge(PRIORITY_UNLIMITED);

    // Tier 2: Free (priority 100-199) - one concurrent job per organization
    // Block if the same org already has a Claimed or Running job
    let tier_free_eligible = priority
        .ge(PRIORITY_FREE)
        .and(priority.lt(PRIORITY_UNLIMITED))
        .and(not(exists(
            job_org
                .filter(
                    job_org
                        .field(status)
                        .eq(JobStatus::Claimed)
                        .or(job_org.field(status).eq(JobStatus::Running)),
                )
                .filter(job_org.field(organization_id).eq(organization_id)),
        )));

    // Tier 3: Unclaimed (priority < 100) - one concurrent job per source IP
    // Block if the same source_ip already has a Claimed or Running job
    let tier_unclaimed_eligible = priority.lt(PRIORITY_FREE).and(not(exists(
        job_ip
            .filter(
                job_ip
                    .field(status)
                    .eq(JobStatus::Claimed)
                    .or(job_ip.field(status).eq(JobStatus::Running)),
            )
            .filter(job_ip.field(source_ip).eq(source_ip)),
    )));

    // Combined eligibility: any tier condition passes
    let eligible = tier_unlimited
        .or(tier_free_eligible)
        .or(tier_unclaimed_eligible);

    // Spec filter: only claim jobs whose spec_id matches one of the runner's specs
    let spec_filter = schema::job::spec_id.eq_any(
        schema::runner_spec::table
            .filter(schema::runner_spec::runner_id.eq(runner_token.runner_id))
            .select(schema::runner_spec::spec_id),
    );

    // Acquire write lock for the entire read-check-update to prevent TOCTOU races
    // where concurrent runners could bypass concurrency limits.
    // Scoped so the lock is released before doing non-DB work (OCI token generation).
    let (query_job, json_spec) = {
        let conn = write_conn!(context);

        // Find the highest-priority eligible pending job matching this runner's specs
        let pending_job: Option<QueryJob> = schema::job::table
            .filter(status.eq(JobStatus::Pending))
            .filter(eligible)
            .filter(spec_filter)
            .order((priority.desc(), created.asc(), id.asc()))
            .first(conn)
            .optional()
            .map_err(|e| {
                HttpError::for_internal_error(format!("Failed to query pending jobs: {e}"))
            })?;

        let Some(query_job) = pending_job else {
            return Ok(None);
        };

        // Claim the job under the same lock
        let now = context.clock.now();
        let update_job = UpdateJob::claim(runner_token.runner_id, now);

        let updated = update_job
            .execute_if_status(conn, query_job.id, JobStatus::Pending)
            .map_err(resource_conflict_err!(Job, query_job))?;

        // Look up the spec before releasing the lock
        let json_spec = (updated > 0)
            .then(|| QuerySpec::get(conn, query_job.spec_id).map(QuerySpec::into_json))
            .transpose()?;

        (query_job, json_spec)
    };

    if let Some(json_spec) = json_spec {
        Ok(Some(build_claimed_job(
            context,
            query_job,
            runner_token,
            json_spec,
        )?))
    } else {
        // Defensive: the UPDATE matched 0 rows despite SELECT finding a pending job.
        // Under the current single-writer lock this should not happen, but we
        // handle it gracefully by returning None (no job claimed this iteration).
        Ok(None)
    }
}

fn build_claimed_job(
    context: &ApiContext,
    query_job: QueryJob,
    runner_token: &RunnerToken,
    json_spec: JsonSpec,
) -> Result<JsonClaimedJob, HttpError> {
    #[cfg(feature = "otel")]
    {
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerJobClaim);

        // Record queue duration (time from creation to claim)
        let now = context.clock.now();
        let queue_duration_secs = query_job.created.elapsed_secs(now);
        let tier = bencher_otel::PriorityTier::from_priority(query_job.priority.into());
        bencher_otel::ApiMeter::record(
            bencher_otel::ApiHistogram::JobQueueDuration(tier),
            queue_duration_secs,
        );
    }

    let timeout = query_job.config.timeout;
    let oci_token = context
        .token_key
        .new_oci_runner(
            runner_token.runner_uuid,
            OCI_RUNNER_TOKEN_TTL,
            Some(query_job.config.project.to_string()),
            vec![OciAction::Pull],
        )
        .map_err(|e| {
            HttpError::for_internal_error(format!("Failed to generate OCI runner token: {e}"))
        })?;

    Ok(JsonClaimedJob {
        uuid: query_job.uuid,
        spec: json_spec,
        config: query_job.config,
        oci_token,
        timeout,
        created: query_job.created,
    })
}

// --- Channel endpoint ---

/// Result of the executing state loop.
enum ExecuteResult {
    /// Job reached a terminal state (Completed/Failed/Canceled).
    JobDone,
    /// Runner disconnected or heartbeat timed out.
    Disconnected,
}

/// Path parameters for the runner channel endpoint.
#[derive(Deserialize, JsonSchema)]
pub struct RunnerChannelParams {
    /// The slug or UUID for a runner.
    pub runner: RunnerResourceId,
}

/// Persistent WebSocket channel for runner lifecycle
///
/// ➕ Bencher Plus: Single persistent WebSocket connection for job assignment
/// and execution. Runner sends `Ready` to request a job, server pushes `Job`
/// or `NoJob`. During execution, handles `Running`, `Heartbeat`, `Completed`,
/// `Failed`, and `Canceled` messages.
/// Authentication is via runner token in the Authorization header.
#[channel {
    protocol = WEBSOCKETS,
    path = "/v0/runners/{runner}/channel",
    tags = ["runners"]
}]
pub async fn runner_channel(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<RunnerChannelParams>,
    conn: WebsocketConnection,
) -> WebsocketChannelResult {
    let context = rqctx.context();
    let log = rqctx.log.clone();
    let path_params = path_params.into_inner();

    // Validate runner token from Authorization header
    let runner_token = RunnerToken::from_request(&rqctx, &path_params.runner).await?;

    // Per-runner rate limiting
    #[cfg(feature = "plus")]
    context
        .rate_limiting
        .runner_request(runner_token.runner_uuid)?;

    // Upgrade to WebSocket
    let mut ws_config = WebSocketConfig::default();
    ws_config.max_message_size = Some(context.request_body_max_bytes);
    ws_config.max_frame_size = Some(context.request_body_max_bytes);
    let ws_stream = tokio_tungstenite::WebSocketStream::from_raw_socket(
        conn.into_inner(),
        Role::Server,
        Some(ws_config),
    )
    .await;

    let (mut tx, mut rx) = ws_stream.split();
    let heartbeat_timeout = context.heartbeat_timeout;

    // State machine: Idle -> Executing -> Idle -> ...
    loop {
        // === IDLE STATE ===
        // Wait for Ready message from runner
        let Ok(poll_timeout) = wait_for_ready(&log, &mut tx, &mut rx, heartbeat_timeout).await
        else {
            break;
        };

        let deadline = tokio::time::Instant::now() + Duration::from_secs(u64::from(poll_timeout));

        // Poll for a job, checking for WS disconnect between polls
        let claimed_job =
            poll_for_job(&log, context, &runner_token, deadline, &mut tx, &mut rx).await;

        match claimed_job {
            Ok(Some(job)) => {
                // Send Job to runner
                let job_uuid = job.uuid;
                let job_msg = ServerMessage::Job(Box::new(job));
                let text = serde_json::to_string(&job_msg)
                    .map_err(|e| HttpError::for_internal_error(e.to_string()))?;
                if tx.send(Message::Text(text.into())).await.is_err() {
                    break;
                }

                // === EXECUTING STATE ===
                let job_db = QueryJob::from_uuid(auth_conn!(context), job_uuid)?;

                match execute_loop(&log, context, &job_db, &mut tx, &mut rx, heartbeat_timeout)
                    .await
                {
                    Ok(ExecuteResult::JobDone) => {
                        // Transition back to Idle
                    },
                    Ok(ExecuteResult::Disconnected) | Err(_) => {
                        // Spawn heartbeat timeout for in-flight jobs
                        let job = QueryJob::get(auth_conn!(context), job_db.id)?;
                        if !job.status.has_run() {
                            slog::info!(log, "Channel disconnected for in-flight job, spawning heartbeat timeout"; "job_id" => ?job.id);
                            spawn_heartbeat_timeout(
                                log,
                                heartbeat_timeout,
                                context.database.connection.clone(),
                                job.id,
                                &context.heartbeat_tasks,
                                context.job_timeout_grace_period,
                                context.clock.clone(),
                            );
                        }
                        break;
                    },
                }
            },
            Ok(None) => {
                // No job available, send NoJob and stay in Idle
                let text = serde_json::to_string(&ServerMessage::NoJob)
                    .map_err(|e| HttpError::for_internal_error(e.to_string()))?;
                if tx.send(Message::Text(text.into())).await.is_err() {
                    break;
                }
            },
            Err(_) => {
                // Error during polling (likely WS disconnect)
                break;
            },
        }
    }

    Ok(())
}

/// Wait for a `RunnerMessage::Ready` message, returning the poll timeout.
///
/// Ignores non-Ready messages with a warning. Returns an error on Close, disconnect,
/// or if the runner stays silent longer than `idle_timeout`.
async fn wait_for_ready<S, R>(
    log: &slog::Logger,
    tx: &mut S,
    rx: &mut R,
    idle_timeout: Duration,
) -> Result<u32, ChannelError>
where
    S: futures::Sink<Message> + Unpin,
    R: futures::Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    loop {
        let Some(msg_result) = (match tokio::time::timeout(idle_timeout, rx.next()).await {
            Ok(msg) => msg,
            Err(_elapsed) => {
                slog::warn!(log, "Idle timeout waiting for Ready message");
                return Err(ChannelError::WebSocket(
                    tokio_tungstenite::tungstenite::Error::ConnectionClosed,
                ));
            },
        }) else {
            return Err(ChannelError::WebSocket(
                tokio_tungstenite::tungstenite::Error::ConnectionClosed,
            ));
        };

        let msg = msg_result?;

        match msg {
            Message::Text(text) => {
                let runner_msg: RunnerMessage = serde_json::from_str(&text)?;
                match runner_msg {
                    RunnerMessage::Ready { poll_timeout } => {
                        let timeout = poll_timeout.map_or(DEFAULT_POLL_TIMEOUT, u32::from);
                        return Ok(timeout);
                    },
                    RunnerMessage::Running
                    | RunnerMessage::Heartbeat
                    | RunnerMessage::Completed { .. }
                    | RunnerMessage::Failed { .. }
                    | RunnerMessage::Canceled => {
                        slog::warn!(log, "Unexpected message in Idle state, expected Ready"; "msg" => ?runner_msg);
                    },
                }
            },
            Message::Close(_) => {
                slog::info!(log, "Channel closed by client during Idle");
                return Err(ChannelError::WebSocket(
                    tokio_tungstenite::tungstenite::Error::ConnectionClosed,
                ));
            },
            Message::Ping(data) => {
                drop(tx.send(Message::Pong(data)).await);
            },
            Message::Binary(_) | Message::Pong(_) | Message::Frame(_) => {},
        }
    }
}

/// Poll for a job, checking for WS disconnect between polls.
///
/// Returns `Ok(Some(job))` if claimed, `Ok(None)` if deadline expired,
/// or `Err` on WS disconnect.
async fn poll_for_job<S, R>(
    log: &slog::Logger,
    context: &ApiContext,
    runner_token: &RunnerToken,
    deadline: tokio::time::Instant,
    tx: &mut S,
    rx: &mut R,
) -> Result<Option<JsonClaimedJob>, ChannelError>
where
    S: futures::Sink<Message> + Unpin,
    R: futures::Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    loop {
        match try_claim_job(context, runner_token).await {
            Ok(Some(job)) => return Ok(Some(job)),
            Ok(None) => {},
            Err(e) => {
                slog::error!(log, "Error claiming job"; "error" => %e);
                // Continue polling — transient DB errors shouldn't break the channel
            },
        }

        if tokio::time::Instant::now() >= deadline {
            return Ok(None);
        }

        // Wait POLL_INTERVAL, but check WS for disconnect
        match tokio::time::timeout(POLL_INTERVAL, rx.next()).await {
            Ok(Some(Ok(Message::Close(_))) | None) => {
                return Err(ChannelError::WebSocket(
                    tokio_tungstenite::tungstenite::Error::ConnectionClosed,
                ));
            },
            Ok(Some(Ok(Message::Ping(data)))) => {
                drop(tx.send(Message::Pong(data)).await);
            },
            Ok(Some(Err(e))) => return Err(ChannelError::WebSocket(e)),
            Ok(Some(Ok(Message::Text(text)))) => {
                slog::warn!(log, "Unexpected text message during polling"; "text" => %text);
            },
            Ok(Some(Ok(Message::Binary(data)))) => {
                slog::warn!(log, "Unexpected binary message during polling"; "len" => data.len());
            },
            Ok(Some(Ok(Message::Pong(_) | Message::Frame(_)))) | Err(_) => {},
        }
    }
}

/// Execute a job on the channel: handle messages until terminal or disconnect.
///
/// Similar to the per-job WS handler but does not send close frames on terminal
/// messages — the connection stays open for the next job cycle.
async fn execute_loop<S, R>(
    log: &slog::Logger,
    context: &ApiContext,
    job: &QueryJob,
    tx: &mut S,
    rx: &mut R,
    heartbeat_timeout: Duration,
) -> Result<ExecuteResult, ChannelError>
where
    S: futures::Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
    R: futures::Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    let mut last_heartbeat = tokio::time::Instant::now();

    loop {
        let remaining = heartbeat_timeout
            .checked_sub(last_heartbeat.elapsed())
            .unwrap_or(Duration::ZERO);

        let msg_result = match tokio::time::timeout(remaining, rx.next()).await {
            Ok(Some(msg_result)) => msg_result,
            Ok(None) => {
                // Stream ended (client disconnected)
                return Ok(ExecuteResult::Disconnected);
            },
            Err(_elapsed) => {
                // Heartbeat timeout — mark job and disconnect
                let _reason = handle_timeout(log, context, job.id).await?;
                return Ok(ExecuteResult::Disconnected);
            },
        };

        let msg = match msg_result {
            Ok(msg) => msg,
            Err(e) => {
                slog::warn!(log, "WebSocket error during execution"; "error" => %e);
                return Ok(ExecuteResult::Disconnected);
            },
        };

        match msg {
            Message::Text(text) => {
                let runner_msg: RunnerMessage = match serde_json::from_str(&text) {
                    Ok(msg) => msg,
                    Err(e) => {
                        slog::warn!(log, "Invalid message"; "error" => %e, "text" => text.to_string());
                        // Do NOT reset heartbeat for invalid JSON
                        continue;
                    },
                };

                // Reset heartbeat on valid protocol messages, but NOT on
                // spurious Ready messages — a misbehaving runner could send
                // periodic Ready to keep the heartbeat alive indefinitely.
                let is_ready = matches!(runner_msg, RunnerMessage::Ready { .. });

                let (response, close_reason) =
                    handle_runner_message(log, context, job, runner_msg).await?;

                if !is_ready {
                    last_heartbeat = tokio::time::Instant::now();
                }

                let response_text = serde_json::to_string(&response)?;
                tx.send(Message::Text(response_text.into())).await?;

                // Terminal message: job done, transition back to Idle
                // (no close frame — connection stays open)
                if close_reason.is_some() {
                    return Ok(ExecuteResult::JobDone);
                }
            },
            Message::Close(_) => {
                slog::info!(log, "Channel closed by client during execution");
                return Ok(ExecuteResult::Disconnected);
            },
            Message::Ping(data) => {
                // Respond to Ping but do NOT reset heartbeat timeout
                tx.send(Message::Pong(data)).await?;
            },
            Message::Binary(_) | Message::Pong(_) | Message::Frame(_) => {
                // Ignore; do NOT reset heartbeat timeout
            },
        }
    }
}
