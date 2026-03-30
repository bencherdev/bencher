//! Persistent WebSocket channel for runner job lifecycle.
//!
//! Single WebSocket connection that handles job assignment, execution status,
//! and stays open between jobs.

use std::time::Duration;

use bencher_billing::CustomerId;
use bencher_json::{
    DEFAULT_POLL_TIMEOUT, JobStatus, JobUuid, JsonClaimedJob, JsonSpec, MeteredPlanId, Priority,
    RunnerResourceId,
    runner::{CloseReason, JsonIterationOutput, RunnerMessage, ServerMessage},
};
use bencher_oci_storage::OciStorageError;
use bencher_schema::{
    auth_conn,
    context::ApiContext,
    error::{resource_conflict_err, resource_not_found_err},
    model::{
        organization::OrganizationId,
        runner::{JobId, QueryJob, RunnerId, UpdateJob, job::spawn_heartbeat_timeout},
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
    protocol::{CloseFrame, Role, WebSocketConfig, frame::coding::CloseCode},
};

use crate::runner_token::RunnerToken;

// --- WebSocket message handlers ---

/// Compute the maximum allowed elapsed seconds for a job: timeout + grace period.
fn job_timeout_limit(timeout: bencher_json::Timeout, grace: Duration) -> i64 {
    let timeout_secs = i64::from(u32::from(timeout));
    #[expect(clippy::cast_possible_wrap, reason = "grace period seconds fit in i64")]
    let grace_secs = grace.as_secs() as i64;
    timeout_secs.saturating_add(grace_secs)
}

/// Decide job status and close reason when a heartbeat times out.
///
/// If the job has run longer than its timeout limit, it is canceled (timeout exceeded).
/// Otherwise it is failed (lost contact with runner).
fn timeout_decision(elapsed_secs: i64, limit_secs: i64) -> (JobStatus, CloseReason) {
    if elapsed_secs > limit_secs {
        (JobStatus::Canceled, CloseReason::JobTimeoutExceeded)
    } else {
        (JobStatus::Failed, CloseReason::HeartbeatTimeout)
    }
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

    #[error("{0}")]
    Billing(#[from] bencher_billing::BillingError),

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
        timeout_decision(elapsed, limit)
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
    billing_state: &mut BillingState,
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
            if let Some(cancel) = handle_heartbeat(log, context, job.id, billing_state).await? {
                return Ok((cancel, Some(CloseReason::JobCanceled)));
            }
        },
        RunnerMessage::Completed {
            job: msg_job_uuid,
            results,
        } => {
            if msg_job_uuid != job.uuid {
                slog::warn!(log, "Completed message job UUID mismatch"; "expected" => %job.uuid, "got" => %msg_job_uuid);
            }
            slog::info!(log, "Job completed"; "job_id" => ?job.id, "iterations" => results.len());
            handle_completed(log, context, job, results).await?;
            bill_final_minutes(log, context, job.id, billing_state).await;
            return Ok((
                ServerMessage::Ack {
                    job: Some(job.uuid),
                },
                Some(CloseReason::JobCompleted),
            ));
        },
        RunnerMessage::Failed {
            job: msg_job_uuid,
            results,
            error,
        } => {
            if msg_job_uuid != job.uuid {
                slog::warn!(log, "Failed message job UUID mismatch"; "expected" => %job.uuid, "got" => %msg_job_uuid);
            }
            slog::warn!(log, "Job failed"; "job_id" => ?job.id, "error" => &error);
            handle_failed(log, context, job, results, error).await?;
            bill_final_minutes(log, context, job.id, billing_state).await;
            return Ok((
                ServerMessage::Ack {
                    job: Some(job.uuid),
                },
                Some(CloseReason::JobFailed),
            ));
        },
        RunnerMessage::Canceled { job: msg_job_uuid } => {
            if msg_job_uuid != job.uuid {
                slog::warn!(log, "Canceled message job UUID mismatch"; "expected" => %job.uuid, "got" => %msg_job_uuid);
            }
            slog::info!(log, "Job cancellation acknowledged"; "job_id" => ?job.id);
            handle_canceled(log, context, job.id).await?;
            bill_final_minutes(log, context, job.id, billing_state).await;
            return Ok((
                ServerMessage::Ack {
                    job: Some(job.uuid),
                },
                Some(CloseReason::JobCanceledByRunner),
            ));
        },
    }

    Ok((
        ServerMessage::Ack {
            job: Some(job.uuid),
        },
        None,
    ))
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
///
/// Uses atomic read+write under `write_conn!` to prevent TOCTOU billing races.
/// Stripe is called best-effort after releasing the write lock.
async fn handle_heartbeat(
    log: &slog::Logger,
    context: &ApiContext,
    job_id: JobId,
    billing_state: &mut BillingState,
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
        let (status, _reason) = timeout_decision(elapsed, limit);
        if status == JobStatus::Canceled {
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

    // Hold the write lock for both read and write to prevent a concurrent
    // call from advancing last_billed_minute between our read and write.
    let bill_info = {
        let conn = write_conn!(context);
        let job = QueryJob::get(conn, job_id)?;

        let billing = BillingDelta::new(&job, now);

        let update = match &billing {
            Some(b) => UpdateJob::heartbeat_with_billing(now, b.minutes),
            None => UpdateJob::heartbeat(now),
        };

        update.execute_if_either_status(conn, job_id, JobStatus::Claimed, JobStatus::Running)?;

        billing
    };
    // write lock released here

    // Bill Stripe best-effort — the delta is already claimed in the DB.
    if let Some(billing) = bill_info {
        bill_stripe_best_effort(
            log,
            context,
            job_id,
            billing.delta,
            billing.organization_id,
            billing_state,
        )
        .await;
    }

    Ok(None)
}

/// Calculate elapsed minutes using ceiling division (bill as soon as any part of a minute starts).
/// Negative input is clamped to zero. Always returns at least 1 minute for non-negative input,
/// so that a job is billed for a minimum of 1 minute even if it completes in under a second.
fn elapsed_minutes(elapsed_secs: i64) -> i32 {
    // Clamp to [0, i32::MAX * 60] so the `+ 59` ceiling-division step cannot overflow
    // and the final value is guaranteed to fit in i32.
    let clamped = elapsed_secs.clamp(0, i64::from(i32::MAX) * 60);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "clamped to i32::MAX * 60 before ceiling division"
    )]
    #[expect(
        clippy::integer_division,
        reason = "intentional ceiling division for minute billing"
    )]
    let minutes = ((clamped + 59) / 60) as i32;
    // Always bill at least 1 minute for any job that ran (including 0 elapsed seconds).
    minutes.max(1)
}

/// Billing delta between elapsed minutes and last billed minute.
#[derive(Debug, PartialEq, Eq)]
struct BillingDelta {
    delta: u32,
    minutes: i32,
    organization_id: OrganizationId,
}

impl BillingDelta {
    /// Calculate the billing delta between elapsed minutes and last billed minute.
    /// Returns `None` if the job hasn't started or no new minutes need billing.
    fn new(job: &QueryJob, now: bencher_json::DateTime) -> Option<Self> {
        let started = job.started?;
        let elapsed_secs = now.timestamp() - started.timestamp();
        let minutes = elapsed_minutes(elapsed_secs);
        let last_billed = job.last_billed_minute.unwrap_or(0);
        if minutes <= last_billed {
            return None;
        }
        #[expect(
            clippy::cast_sign_loss,
            reason = "delta is always positive (minutes > last_billed)"
        )]
        let delta = (minutes - last_billed) as u32;
        Some(Self {
            delta,
            minutes,
            organization_id: job.organization_id,
        })
    }
}

/// Report runner usage to Stripe best-effort. Errors are logged but not propagated.
async fn bill_stripe_best_effort(
    log: &slog::Logger,
    context: &ApiContext,
    job_id: JobId,
    delta: u32,
    organization_id: OrganizationId,
    billing_state: &mut BillingState,
) {
    let customer_id = match billing_state.customer_id(context, organization_id).await {
        Ok(Some(id)) => id,
        Ok(None) => return,
        Err(e) => {
            slog::warn!(log, "Failed to look up customer for billing"; "job_id" => ?job_id, "error" => %e);
            #[cfg(feature = "sentry")]
            sentry::capture_error(&e);
            return;
        },
    };

    let Some(biller) = context.biller.as_ref() else {
        return;
    };

    if let Err(e) = biller.record_runner_usage(&customer_id, delta).await {
        slog::warn!(log, "Failed to record runner billing"; "job_id" => ?job_id, "delta" => delta, "error" => %e);
        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerMinutesBilledFailed);
        #[cfg(feature = "sentry")]
        billing_state.report_err(&e);
    } else {
        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerMinutesBilled);
    }
}

/// Cached result of the customer ID lookup for an organization.
///
/// Avoids querying the DB and Stripe on every heartbeat, since the customer
/// will not change mid-job.
enum CachedCustomer {
    /// Not yet looked up.
    Unknown,
    /// Looked up and no metered plan exists for the organization.
    None,
    /// Looked up and resolved the Stripe customer ID.
    Some(CustomerId),
}

/// Per-job billing state: caches the customer ID lookup and tracks Sentry
/// reporting so only the first billing failure per job is sent.
///
/// Created at the start of each job execution and dropped when the job finishes.
struct BillingState {
    customer: CachedCustomer,
    #[cfg(feature = "sentry")]
    reported: bool,
}

impl BillingState {
    fn new() -> Self {
        Self {
            customer: CachedCustomer::Unknown,
            #[cfg(feature = "sentry")]
            reported: false,
        }
    }

    /// Return the cached customer ID, querying the DB and Stripe on first call.
    async fn customer_id(
        &mut self,
        context: &ApiContext,
        organization_id: OrganizationId,
    ) -> Result<Option<CustomerId>, ChannelError> {
        match &self.customer {
            CachedCustomer::Unknown => {
                let plan_id: Option<Option<MeteredPlanId>> = schema::plan::table
                    .filter(schema::plan::organization_id.eq(organization_id))
                    .select(schema::plan::metered_plan)
                    .first(auth_conn!(context))
                    .optional()?;
                if let Some(metered_plan_id) = plan_id.flatten() {
                    let Some(biller) = context.biller.as_ref() else {
                        self.customer = CachedCustomer::None;
                        return Ok(None);
                    };
                    let (status, customer_id) =
                        biller.get_metered_plan_status(&metered_plan_id).await?;
                    if status.is_active() {
                        self.customer = CachedCustomer::Some(customer_id.clone());
                        Ok(Some(customer_id))
                    } else {
                        self.customer = CachedCustomer::None;
                        Ok(None)
                    }
                } else {
                    self.customer = CachedCustomer::None;
                    Ok(None)
                }
            },
            CachedCustomer::None => Ok(None),
            CachedCustomer::Some(id) => Ok(Some(id.clone())),
        }
    }

    /// Report a billing failure to Sentry (first failure only).
    #[cfg(feature = "sentry")]
    fn report_err(&mut self, error: &bencher_billing::BillingError) {
        if !self.reported {
            self.reported = true;
            sentry::capture_error(error);
        }
    }
}

/// Final billing call for terminal job states.
///
/// Re-reads the job from the database to get the latest `last_billed_minute`
/// (which may have been updated by prior heartbeats), bills any remaining
/// partial-minute delta, and persists the new `last_billed_minute`.
///
/// This ensures that if a job completes mid-minute (e.g., at 90 seconds),
/// the final partial minute is billed even when no heartbeat fires between
/// the last billed minute and completion.
///
/// Logs and ignores errors so that a transient DB or billing failure
/// does not prevent job completion.
async fn bill_final_minutes(
    log: &slog::Logger,
    context: &ApiContext,
    job_id: JobId,
    billing_state: &mut BillingState,
) {
    if let Err(e) = bill_final_minutes_inner(log, context, job_id, billing_state).await {
        slog::error!(log, "Final billing failed"; "job_id" => ?job_id, "error" => %e);
        #[cfg(feature = "sentry")]
        sentry::capture_error(&e);
    }
}

async fn bill_final_minutes_inner(
    log: &slog::Logger,
    context: &ApiContext,
    job_id: JobId,
    billing_state: &mut BillingState,
) -> Result<(), ChannelError> {
    let now = context.clock.now();

    // Hold the write lock for both read and write to prevent a concurrent
    // heartbeat from advancing last_billed_minute between our read and write.
    let bill_info = {
        let conn = write_conn!(context);
        let job = QueryJob::get(conn, job_id)?;

        let Some(billing) = BillingDelta::new(&job, now) else {
            return Ok(());
        };

        let update_job = UpdateJob::final_billing(billing.minutes, now);
        // No status filter needed here (unlike `handle_heartbeat` which uses
        // `execute_if_either_status`): the write lock held since `write_conn!`
        // prevents concurrent heartbeats from advancing `last_billed_minute`
        // between our read and write, and `UpdateJob::final_billing` only
        // touches `last_billed_minute`/`modified` — not status — so it is safe
        // regardless of the current job status.
        update_job.execute(conn, job_id)?;

        billing
    };
    // write lock released here

    // Bill Stripe best-effort — the delta is already claimed in the DB.
    bill_stripe_best_effort(
        log,
        context,
        job_id,
        bill_info.delta,
        bill_info.organization_id,
        billing_state,
    )
    .await;

    Ok(())
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

    // Allow Failed → Completed: a runner that finishes successfully after the
    // server marked the job Failed (e.g., heartbeat timeout fired while the
    // runner was still completing) should be permitted to override the status.
    // The runner is the authority on whether the benchmark actually succeeded.
    let updated = update.execute_if_either_status(
        write_conn!(context),
        job.id,
        JobStatus::Running,
        JobStatus::Failed,
    )?;

    // Re-read job to get fresh timestamps (started, claimed) set during execution.
    // The in-memory QueryJob was loaded at claim time before these fields were set.
    let job = QueryJob::get(auth_conn!(context), job.id)?;

    if updated == 0 {
        if matches!(job.status, JobStatus::Completed | JobStatus::Processed) {
            slog::debug!(log, "Job already completed (idempotent duplicate)"; "job_id" => ?job.id);
            return Ok(());
        }
        if job.status == JobStatus::Canceled {
            slog::warn!(log, "Job already canceled, completion report lost"; "job_id" => ?job.id);
            return Ok(());
        }
        return Err(ChannelError::InvalidStateTransition {
            job_id: job.id,
            target: JobStatus::Completed,
            current: job.status,
        });
    }

    #[cfg(feature = "otel")]
    {
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerJobUpdate(
            bencher_otel::JobStatusKind::Completed,
        ));

        let duration_secs = job.created.elapsed_secs(now);
        bencher_otel::ApiMeter::record(
            bencher_otel::ApiHistogram::JobCompleteDuration(job.priority),
            duration_secs,
        );
    }

    // Store output in blob storage (best-effort)
    let job_output = bencher_json::runner::JsonJobOutput {
        results,
        error: None,
    };
    if let Err(e) = store_job_output(context, &job, &job_output).await {
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

/// Poll interval for long-polling (1 second)
const POLL_INTERVAL: Duration = Duration::from_secs(1);

/// Attempt to claim a pending job with tier-based concurrency limits.
///
/// Priority tiers:
/// - Plus: Unlimited concurrent jobs
/// - Free: 1 concurrent job per organization
/// - Unclaimed: 1 concurrent job per source IP
///
/// Returns `Ok(Some(job))` if a job was claimed, `Ok(None)` if no eligible jobs available.
/// OCI runner token TTL: 10 minutes (enough for image pull, short enough to limit exposure)
const OCI_RUNNER_TOKEN_TTL: u32 = 600;

async fn try_claim_job(
    context: &ApiContext,
    runner_token: &RunnerToken,
) -> Result<Option<(QueryJob, JsonClaimedJob)>, HttpError> {
    use schema::job::dsl::{created, id, organization_id, priority, source_ip, status};

    // Tier 1: Plus (priority >= 200) - no concurrency limit
    let tier_unlimited = priority.ge(Priority::Plus);

    // Tier 2: Free (priority 100-199) - one concurrent job per organization
    // Block if the same org already has a Claimed or Running job
    let tier_free_eligible = priority
        .ge(Priority::Free)
        .and(priority.lt(Priority::Plus))
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
    let tier_unclaimed_eligible = priority.lt(Priority::Free).and(not(exists(
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
        let claimed_job = build_claimed_job(context, query_job.clone(), runner_token, json_spec)?;
        Ok(Some((query_job, claimed_job)))
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
        bencher_otel::ApiMeter::record(
            bencher_otel::ApiHistogram::JobQueueDuration(query_job.priority),
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
#[expect(clippy::too_many_lines)]
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
        // Wait for Ready message from runner (also handles terminal message retries)
        let Ok(poll_timeout) = wait_for_ready(
            &log,
            context,
            runner_token.runner_id,
            &mut tx,
            &mut rx,
            heartbeat_timeout,
        )
        .await
        else {
            break;
        };

        let deadline = tokio::time::Instant::now() + Duration::from_secs(u64::from(poll_timeout));

        // Poll for a job, checking for WS disconnect between polls
        let claimed_job =
            poll_for_job(&log, context, &runner_token, deadline, &mut tx, &mut rx).await;

        match claimed_job {
            Ok(Some((query_job, claimed_job))) => {
                // Send Job to runner
                let job_msg = ServerMessage::Job(Box::new(claimed_job));
                let text = serde_json::to_string(&job_msg)
                    .map_err(|e| HttpError::for_internal_error(e.to_string()))?;
                if tx.send(Message::Text(text.into())).await.is_err() {
                    break;
                }

                // === EXECUTING STATE ===

                match execute_loop(
                    &log,
                    context,
                    &query_job,
                    &mut tx,
                    &mut rx,
                    heartbeat_timeout,
                )
                .await
                {
                    Ok(ExecuteResult::JobDone) => {
                        // Transition back to Idle
                    },
                    Ok(ExecuteResult::Disconnected) => {
                        // Spawn heartbeat timeout for in-flight jobs
                        let job = QueryJob::get(auth_conn!(context), query_job.id)?;
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
                    Err(e) => {
                        slog::error!(log, "Execute loop error"; "error" => %e, "job_id" => ?query_job.id);
                        // Spawn heartbeat timeout for in-flight jobs
                        let job = QueryJob::get(auth_conn!(context), query_job.id)?;
                        if !job.status.has_run() {
                            slog::info!(log, "Channel error for in-flight job, spawning heartbeat timeout"; "job_id" => ?job.id);
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
/// Also handles terminal messages (Completed/Failed/Canceled) during Idle state,
/// which occur when a runner reconnects with a pending result that wasn't acknowledged.
/// Returns an error on Close, disconnect, or if the runner stays silent longer
/// than `idle_timeout`.
async fn wait_for_ready<S, R>(
    log: &slog::Logger,
    context: &ApiContext,
    runner_id: RunnerId,
    tx: &mut S,
    rx: &mut R,
    idle_timeout: Duration,
) -> Result<u32, ChannelError>
where
    S: futures::Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
    R: futures::Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    loop {
        let msg = match tokio::time::timeout(idle_timeout, rx.next()).await {
            Ok(Some(msg)) => msg,
            Ok(None) => {
                return Err(ChannelError::WebSocket(
                    tokio_tungstenite::tungstenite::Error::ConnectionClosed,
                ));
            },
            Err(_elapsed) => {
                slog::warn!(log, "Idle timeout waiting for Ready message");
                return Err(ChannelError::WebSocket(
                    tokio_tungstenite::tungstenite::Error::ConnectionClosed,
                ));
            },
        };

        let msg = msg?;

        match msg {
            Message::Text(text) => {
                let runner_msg: RunnerMessage = serde_json::from_str(&text)?;
                match runner_msg {
                    RunnerMessage::Ready { poll_timeout } => {
                        let timeout = poll_timeout.map_or(DEFAULT_POLL_TIMEOUT, u32::from);
                        return Ok(timeout);
                    },
                    RunnerMessage::Completed {
                        job: job_uuid,
                        results,
                    } => {
                        slog::info!(log, "Received Completed during Idle (retry after reconnect)"; "job_uuid" => %job_uuid);
                        if let Some(job) =
                            lookup_runner_job(log, context, runner_id, job_uuid).await?
                        {
                            handle_completed(log, context, &job, results).await?;
                        }
                        let ack = serde_json::to_string(&ServerMessage::Ack {
                            job: Some(job_uuid),
                        })?;
                        tx.send(Message::Text(ack.into())).await?;
                    },
                    RunnerMessage::Failed {
                        job: job_uuid,
                        results,
                        error,
                    } => {
                        slog::info!(log, "Received Failed during Idle (retry after reconnect)"; "job_uuid" => %job_uuid, "error" => &error);
                        if let Some(job) =
                            lookup_runner_job(log, context, runner_id, job_uuid).await?
                        {
                            handle_failed(log, context, &job, results, error).await?;
                        }
                        let ack = serde_json::to_string(&ServerMessage::Ack {
                            job: Some(job_uuid),
                        })?;
                        tx.send(Message::Text(ack.into())).await?;
                    },
                    RunnerMessage::Canceled { job: job_uuid } => {
                        slog::info!(log, "Received Canceled during Idle (retry after reconnect)"; "job_uuid" => %job_uuid);
                        if let Some(job) =
                            lookup_runner_job(log, context, runner_id, job_uuid).await?
                        {
                            handle_canceled(log, context, job.id).await?;
                        }
                        let ack = serde_json::to_string(&ServerMessage::Ack {
                            job: Some(job_uuid),
                        })?;
                        tx.send(Message::Text(ack.into())).await?;
                    },
                    RunnerMessage::Running | RunnerMessage::Heartbeat => {
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
                if let Err(e) = tx.send(Message::Pong(data)).await {
                    slog::warn!(log, "Failed to send pong"; "error" => %e);
                }
            },
            Message::Binary(_) | Message::Pong(_) | Message::Frame(_) => {},
        }
    }
}

/// Look up a job by UUID and verify it belongs to the given runner.
///
/// Returns `Ok(Some(job))` if found and owned by this runner,
/// `Ok(None)` if not found or owned by a different runner (with a warning log).
async fn lookup_runner_job(
    log: &slog::Logger,
    context: &ApiContext,
    runner_id: RunnerId,
    job_uuid: JobUuid,
) -> Result<Option<QueryJob>, ChannelError> {
    let job: Option<QueryJob> = schema::job::table
        .filter(schema::job::uuid.eq(job_uuid))
        .first(auth_conn!(context))
        .optional()?;

    let Some(job) = job else {
        slog::warn!(log, "Terminal message for unknown job during Idle"; "job_uuid" => %job_uuid);
        return Ok(None);
    };

    if job.runner_id != Some(runner_id) {
        slog::warn!(log, "Terminal message for job not owned by this runner"; "job_uuid" => %job_uuid);
        return Ok(None);
    }

    Ok(Some(job))
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
) -> Result<Option<(QueryJob, JsonClaimedJob)>, ChannelError>
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
                if tx.send(Message::Pong(data)).await.is_err() {
                    slog::warn!(log, "Failed to send pong during polling");
                }
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
    let mut billing_state = BillingState::new();

    loop {
        let remaining = heartbeat_timeout
            .checked_sub(last_heartbeat.elapsed())
            .unwrap_or(Duration::ZERO);

        let msg_result = match tokio::time::timeout(remaining, rx.next()).await {
            Ok(Some(msg_result)) => msg_result,
            Ok(None) => {
                // Stream ended (client disconnected) — best-effort bill any remaining partial minute
                bill_final_minutes(log, context, job.id, &mut billing_state).await;
                return Ok(ExecuteResult::Disconnected);
            },
            Err(_elapsed) => {
                // Heartbeat timeout — best-effort bill any remaining partial minute before marking job
                bill_final_minutes(log, context, job.id, &mut billing_state).await;
                let reason = handle_timeout(log, context, job.id).await?;
                let reason_json = serde_json::to_string(&reason)?;
                let close_frame = CloseFrame {
                    code: CloseCode::Normal,
                    reason: reason_json.into(),
                };
                drop(tx.send(Message::Close(Some(close_frame))).await);
                return Ok(ExecuteResult::Disconnected);
            },
        };

        let msg = match msg_result {
            Ok(msg) => msg,
            Err(e) => {
                slog::warn!(log, "WebSocket error during execution"; "error" => %e);
                bill_final_minutes(log, context, job.id, &mut billing_state).await;
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
                    handle_runner_message(log, context, job, runner_msg, &mut billing_state)
                        .await?;

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
                bill_final_minutes(log, context, job.id, &mut billing_state).await;
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

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};

    use bencher_json::{DateTime, Timeout};
    use bencher_schema::model::{project::report::ReportId, runner::SourceIp, spec::SpecId};

    use super::*;

    fn test_job() -> QueryJob {
        QueryJob {
            id: JobId::try_from_raw(1).unwrap(),
            uuid: JobUuid::default(),
            report_id: ReportId::try_from_raw(1).unwrap(),
            organization_id: OrganizationId::try_from_raw(1).unwrap(),
            source_ip: SourceIp::new(IpAddr::V4(Ipv4Addr::LOCALHOST)),
            spec_id: SpecId::try_from_raw(1).unwrap(),
            config: serde_json::from_value(serde_json::json!({
                "registry": "https://registry.example.com",
                "project": "00000000-0000-0000-0000-000000000000",
                "digest": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
                "timeout": 300
            }))
            .unwrap(),
            timeout: Timeout::MIN,
            priority: Priority::Plus,
            status: JobStatus::Running,
            runner_id: Some(RunnerId::try_from_raw(1).unwrap()),
            claimed: Some(DateTime::TEST),
            started: Some(DateTime::TEST),
            completed: None,
            last_heartbeat: Some(DateTime::TEST),
            last_billed_minute: None,
            created: DateTime::TEST,
            modified: DateTime::TEST,
        }
    }

    #[test]
    fn timeout_exceeded_cancels_job() {
        let (status, reason) = timeout_decision(301, 300);
        assert_eq!(status, JobStatus::Canceled);
        assert_eq!(reason, CloseReason::JobTimeoutExceeded);
    }

    #[test]
    fn within_timeout_fails_job() {
        let (status, reason) = timeout_decision(100, 300);
        assert_eq!(status, JobStatus::Failed);
        assert_eq!(reason, CloseReason::HeartbeatTimeout);
    }

    #[test]
    fn exactly_at_limit_fails_job() {
        let (status, reason) = timeout_decision(300, 300);
        assert_eq!(status, JobStatus::Failed);
        assert_eq!(reason, CloseReason::HeartbeatTimeout);
    }

    // --- elapsed_minutes tests (ceil division) ---

    #[test]
    fn elapsed_minutes_zero_seconds() {
        // Always bill at least 1 minute, even for a zero-second job
        assert_eq!(elapsed_minutes(0), 1);
    }

    #[test]
    fn elapsed_minutes_one_second() {
        // Ceil: 1 second into a minute still counts as 1 minute
        assert_eq!(elapsed_minutes(1), 1);
    }

    #[test]
    fn elapsed_minutes_fifty_nine_seconds() {
        assert_eq!(elapsed_minutes(59), 1);
    }

    #[test]
    fn elapsed_minutes_sixty_seconds() {
        // Exactly 1 minute
        assert_eq!(elapsed_minutes(60), 1);
    }

    #[test]
    fn elapsed_minutes_sixty_one_seconds() {
        // 1 second into the second minute
        assert_eq!(elapsed_minutes(61), 2);
    }

    #[test]
    fn elapsed_minutes_two_full_minutes() {
        assert_eq!(elapsed_minutes(120), 2);
    }

    #[test]
    fn elapsed_minutes_negative_seconds() {
        // Negative input is clamped to zero, then the 1-minute minimum applies
        assert_eq!(elapsed_minutes(-1), 1);
        assert_eq!(elapsed_minutes(-100), 1);
    }

    #[test]
    fn elapsed_minutes_i64_max() {
        assert_eq!(elapsed_minutes(i64::MAX), i32::MAX);
    }

    // --- BillingDelta tests ---

    #[test]
    fn billing_delta_not_started() {
        let mut job = test_job();
        job.started = None;
        assert_eq!(BillingDelta::new(&job, DateTime::TEST), None);
    }

    #[test]
    fn billing_delta_first_minute() {
        // 0 elapsed seconds → 1 minute (ceiling), no last_billed → delta = 1
        let job = test_job();
        assert_eq!(
            BillingDelta::new(&job, DateTime::TEST),
            Some(BillingDelta {
                delta: 1,
                minutes: 1,
                organization_id: job.organization_id
            }),
        );
    }

    #[test]
    fn billing_delta_already_billed() {
        // Already billed minute 1, still in minute 1 → None
        let mut job = test_job();
        job.last_billed_minute = Some(1);
        assert_eq!(BillingDelta::new(&job, DateTime::TEST), None);
    }

    #[test]
    fn billing_delta_exactly_one_minute() {
        // 60 seconds → minute 1, last_billed = None (0) → delta = 1
        let job = test_job();
        let now = DateTime::try_from(DateTime::TEST.timestamp() + 60).unwrap();
        assert_eq!(
            BillingDelta::new(&job, now),
            Some(BillingDelta {
                delta: 1,
                minutes: 1,
                organization_id: job.organization_id
            })
        );
    }

    #[test]
    fn billing_delta_into_second_minute() {
        // 61 seconds → minute 2, last_billed = None (0) → delta = 2
        let job = test_job();
        let now = DateTime::try_from(DateTime::TEST.timestamp() + 61).unwrap();
        assert_eq!(
            BillingDelta::new(&job, now),
            Some(BillingDelta {
                delta: 2,
                minutes: 2,
                organization_id: job.organization_id
            })
        );
    }

    #[test]
    fn billing_delta_partial_catch_up() {
        // 180 seconds → minute 3, last_billed = 1 → delta = 2
        let mut job = test_job();
        job.last_billed_minute = Some(1);
        let now = DateTime::try_from(DateTime::TEST.timestamp() + 180).unwrap();
        assert_eq!(
            BillingDelta::new(&job, now),
            Some(BillingDelta {
                delta: 2,
                minutes: 3,
                organization_id: job.organization_id
            })
        );
    }

    #[test]
    fn billing_delta_last_billed_equals_minutes() {
        // 120 seconds → minute 2, last_billed = 2 → None
        let mut job = test_job();
        job.last_billed_minute = Some(2);
        let now = DateTime::try_from(DateTime::TEST.timestamp() + 120).unwrap();
        assert_eq!(BillingDelta::new(&job, now), None);
    }

    #[test]
    fn billing_delta_last_billed_exceeds_minutes() {
        // 120 seconds → minute 2, last_billed = 5 → None
        // (can happen if clock skew or final billing already ran)
        let mut job = test_job();
        job.last_billed_minute = Some(5);
        let now = DateTime::try_from(DateTime::TEST.timestamp() + 120).unwrap();
        assert_eq!(BillingDelta::new(&job, now), None);
    }

    #[test]
    fn billing_delta_preserves_organization_id() {
        let mut job = test_job();
        job.organization_id = OrganizationId::try_from_raw(42).unwrap();
        assert_eq!(
            BillingDelta::new(&job, DateTime::TEST),
            Some(BillingDelta {
                delta: 1,
                minutes: 1,
                organization_id: OrganizationId::try_from_raw(42).unwrap()
            }),
        );
    }

    #[test]
    fn billing_delta_large_elapsed() {
        // 1 hour = 3600 seconds → minute 60, last_billed = 58 → delta = 2
        let mut job = test_job();
        job.last_billed_minute = Some(58);
        let now = DateTime::try_from(DateTime::TEST.timestamp() + 3600).unwrap();
        assert_eq!(
            BillingDelta::new(&job, now),
            Some(BillingDelta {
                delta: 2,
                minutes: 60,
                organization_id: job.organization_id
            })
        );
    }
}
