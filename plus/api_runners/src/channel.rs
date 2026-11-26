//! Persistent WebSocket channel for runner job lifecycle.
//!
//! Single WebSocket connection that handles job assignment, execution status,
//! and stays open between jobs.

use std::time::Duration;

use bencher_billing::CustomerId;
#[cfg(feature = "otel")]
use bencher_json::runner::PauseReason;
use bencher_json::{
    DEFAULT_POLL_TIMEOUT, Iteration, JobStatus, JobUuid, JsonClaimedJob, JsonSpec, MeteredPlanId,
    Priority, RunnerResourceId, Timeout,
    runner::{
        CloseReason, JsonIterationOutput, JsonJobOutput, JsonPaused, MAX_PAUSE_REASONS,
        RunnerMessage, ServerMessage,
    },
};
use bencher_oci_storage::OciStorageError;
use bencher_schema::{
    auth_conn,
    context::{ApiContext, CancellationToken},
    error::resource_conflict_err,
    model::{
        organization::OrganizationId,
        project::report::results::ParseRecord,
        runner::{
            ExecutionStatus, InsertJobExecution, JobExecutionId, JobExecutions, JobId, JobTimeout,
            QueryJob, QueryJobExecution, QueryRunner, QueryRunnerStatus, RunnerId, RunnerReport,
            StatusChange, UpdateJob, UpdateJobExecution, end_job, end_job_now, execution_runs,
            finish_failed_job, job_execution::is_in_progress, settle_job, sync_job_status,
        },
        spec::QuerySpec,
    },
    schema, write_conn, write_transaction,
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

use crate::runner_key::RunnerAuth;

// --- WebSocket message handlers ---

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

    #[error("{0}")]
    Storage(#[from] OciStorageError),

    /// An execution is in an unexpected state for the requested transition.
    #[error(
        "Invalid state transition to {target:?} for execution {execution_id:?} of job {job_id:?}, found {current:?}"
    )]
    InvalidStateTransition {
        job_id: JobId,
        execution_id: JobExecutionId,
        target: ExecutionStatus,
        current: ExecutionStatus,
    },

    #[error("Failed to parse runner download URL: {0}")]
    UpdateUrl(url::ParseError),

    #[error("Failed to fetch runner checksum: {0}")]
    UpdateHttp(reqwest::Error),

    #[error("Checksum fetch returned non-success status: {0}")]
    UpdateHttpStatus(reqwest::StatusCode),

    #[error("Checksum response too large: {0} bytes")]
    UpdateChecksumTooLarge(usize),

    #[error("Checksum response is not valid UTF-8: {0}")]
    UpdateChecksumUtf8(std::str::Utf8Error),

    #[error("Empty checksum response")]
    UpdateEmptyChecksum,

    #[error("Invalid runner checksum: {0}")]
    UpdateChecksum(bencher_json::ValidError),
}

/// Handle a heartbeat timeout by reading the execution and its job and deciding the right status.
///
/// If the job's executions have run longer than its timeout plus the grace period, or this
/// execution longer than its own budget plus the grace period, the job is marked `Canceled`
/// (ran too long). Otherwise the execution is marked `Unknown` (lost contact with runner).
/// Returns the [`CloseReason`] for the WebSocket close frame.
async fn handle_timeout(
    log: &slog::Logger,
    context: &ApiContext,
    execution_id: JobExecutionId,
) -> Result<CloseReason, ChannelError> {
    let grace_period = context.job_timeout_grace_period;
    let conn = write_conn!(context);
    let now = context.clock.now();
    let execution = QueryJobExecution::get(conn, execution_id)?;
    let job_executions = JobExecutions::get(conn, execution.job_id)?;
    let job_id = execution.job_id;

    if !execution.status.is_in_flight() || !is_in_progress(job_executions.job.status) {
        slog::info!(log, "Heartbeat timeout: execution already in terminal state"; "execution_id" => ?execution_id, "job_id" => ?job_id);
        return Ok(CloseReason::HeartbeatTimeout);
    }

    if job_executions.is_past_deadline(now, grace_period)
        || execution.is_past_deadline(now, grace_period)
    {
        slog::warn!(log, "Marking job"; "job_id" => ?job_id, "status" => ?JobStatus::Canceled, "reason" => ?CloseReason::JobTimeoutExceeded);
        if end_job(conn, job_id, JobStatus::Canceled, now)? {
            context.callbacks.fire(log, job_id);
        } else {
            slog::info!(log, "Timeout: job already changed state"; "job_id" => ?job_id);
        }
        return Ok(CloseReason::JobTimeoutExceeded);
    }

    slog::warn!(log, "Marking execution"; "execution_id" => ?execution_id, "job_id" => ?job_id, "status" => ?ExecutionStatus::Unknown, "reason" => ?CloseReason::HeartbeatTimeout);
    let updated = UpdateJobExecution::set_status(ExecutionStatus::Unknown).execute_if_any_status(
        conn,
        execution_id,
        &[ExecutionStatus::Sent, ExecutionStatus::Running],
    )?;
    if updated > 0 {
        sync_job_status(conn, job_id, now)?;
        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerJobUpdate(
            bencher_otel::JobStatusKind::Unknown,
        ));
    } else {
        slog::info!(log, "Timeout: execution already changed state"; "execution_id" => ?execution_id);
    }
    Ok(CloseReason::HeartbeatTimeout)
}

/// Handle a message from the runner and return the appropriate response
/// along with an optional [`CloseReason`] for terminal messages.
async fn handle_runner_message(
    log: &slog::Logger,
    context: &ApiContext,
    execution: &QueryJobExecution,
    msg: RunnerMessage,
    billing_state: &mut BillingState,
) -> Result<(ServerMessage, Option<CloseReason>), ChannelError> {
    let ack = ServerMessage::Ack {
        job: Some(execution.uuid),
    };
    let (result, close_reason) = match msg {
        RunnerMessage::Ready(_) | RunnerMessage::Paused(_) => {
            slog::warn!(log, "Unexpected Ready or Paused message during job execution"; "job_id" => ?execution.job_id);
            // Ack is the only safe response: Cancel would terminate the job.
            // The heartbeat timer is NOT reset for Ready or Paused (handled by caller).
            return Ok((ack, None));
        },
        RunnerMessage::Running => {
            slog::info!(log, "Job running"; "job_id" => ?execution.job_id, "execution_id" => ?execution.id);
            if let Some(cancel) = handle_running(log, context, execution.id).await? {
                return Ok((cancel, Some(CloseReason::JobCanceled)));
            }
            return Ok((ack, None));
        },
        RunnerMessage::Heartbeat => {
            slog::debug!(log, "Job heartbeat"; "job_id" => ?execution.job_id, "execution_id" => ?execution.id);
            if let Some(cancel) =
                handle_heartbeat(log, context, execution.id, billing_state).await?
            {
                return Ok((cancel, Some(CloseReason::JobCanceled)));
            }
            return Ok((ack, None));
        },
        RunnerMessage::Completed { job, results } => {
            warn_if_mismatched(log, "Completed", execution, job);
            slog::info!(log, "Execution completed"; "job_id" => ?execution.job_id, "execution_id" => ?execution.id, "iterations" => results.len());
            (RunResult::Completed(results), CloseReason::JobCompleted)
        },
        RunnerMessage::Failed {
            job,
            results,
            error,
        } => {
            warn_if_mismatched(log, "Failed", execution, job);
            slog::warn!(log, "Execution failed"; "job_id" => ?execution.job_id, "execution_id" => ?execution.id, "error" => &error);
            (RunResult::Failed { results, error }, CloseReason::JobFailed)
        },
        RunnerMessage::Canceled { job } => {
            warn_if_mismatched(log, "Canceled", execution, job);
            slog::info!(log, "Job cancellation acknowledged"; "job_id" => ?execution.job_id, "execution_id" => ?execution.id);
            (RunResult::Canceled, CloseReason::JobCanceledByRunner)
        },
    };
    handle_result(log, context, execution.id, result).await?;
    bill_final_minutes(log, context, execution.job_id, billing_state).await;
    Ok((ack, Some(close_reason)))
}

/// A runner names the execution it reports on, which on its own channel is always the one it
/// was sent there.
fn warn_if_mismatched(
    log: &slog::Logger,
    message: &str,
    execution: &QueryJobExecution,
    named: JobUuid,
) {
    if named != execution.uuid {
        slog::warn!(log, "Message execution UUID mismatch"; "message" => message, "expected" => %execution.uuid, "got" => %named);
    }
}

/// Handle a Running message: transition the execution from Sent to Running, and its job from
/// Claimed to Running on its first execution, or update the heartbeat if the execution is already
/// Running (reconnection case). An execution whose job already has its outcome gets `Cancel`.
///
/// Uses conditional UPDATEs with status filters to avoid TOCTOU races.
async fn handle_running(
    log: &slog::Logger,
    context: &ApiContext,
    execution_id: JobExecutionId,
) -> Result<Option<ServerMessage>, ChannelError> {
    let now = context.clock.now();
    let conn = write_conn!(context);
    let execution = QueryJobExecution::get(conn, execution_id)?;
    let job = QueryJob::get(conn, execution.job_id)?;

    if execution.status == ExecutionStatus::Canceled || job.status.has_run() {
        slog::info!(log, "Job has its outcome before the Running transition"; "job_id" => ?job.id, "status" => ?job.status);
        return Ok(Some(ServerMessage::Cancel));
    }

    match execution.status {
        ExecutionStatus::Running => {
            UpdateJobExecution::heartbeat(now).execute_if_any_status(
                conn,
                execution_id,
                &[ExecutionStatus::Running],
            )?;
            UpdateJob::heartbeat(now).execute_if_status(conn, job.id, JobStatus::Running)?;
            slog::info!(log, "Runner reconnected to running job"; "job_id" => ?job.id);
        },
        ExecutionStatus::Sent => {
            UpdateJobExecution::start(now).execute_if_any_status(
                conn,
                execution_id,
                &[ExecutionStatus::Sent],
            )?;
            // The job starts with its first execution.
            let started =
                UpdateJob::start(now).execute_if_status(conn, job.id, JobStatus::Claimed)?;
            if started > 0 {
                #[cfg(feature = "otel")]
                bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerJobUpdate(
                    bencher_otel::JobStatusKind::Running,
                ));
            } else {
                UpdateJob::heartbeat(now).execute_if_status(conn, job.id, JobStatus::Running)?;
            }
        },
        ExecutionStatus::Completed
        | ExecutionStatus::Failed
        | ExecutionStatus::Canceled
        | ExecutionStatus::Unknown => {
            slog::warn!(log, "Unexpected execution state during Running transition"; "job_id" => ?job.id, "status" => ?execution.status);
        },
    }
    Ok(None)
}

/// Handle a Heartbeat message: update the execution's `last_heartbeat` and check for
/// cancellation and timeout.
///
/// Uses atomic read+write under `write_conn!` to prevent TOCTOU billing races.
/// Stripe is called best-effort after releasing the write lock.
async fn handle_heartbeat(
    log: &slog::Logger,
    context: &ApiContext,
    execution_id: JobExecutionId,
    billing_state: &mut BillingState,
) -> Result<Option<ServerMessage>, ChannelError> {
    let now = context.clock.now();
    let grace_period = context.job_timeout_grace_period;

    // Hold the write lock for both read and write to prevent a concurrent
    // call from advancing last_billed_minute between our read and write.
    let bill_info = {
        let conn = write_conn!(context);
        let execution = QueryJobExecution::get(conn, execution_id)?;
        let job_executions = JobExecutions::get(conn, execution.job_id)?;
        let job = &job_executions.job;

        // The job has its outcome, from this execution or another
        if execution.status == ExecutionStatus::Canceled || job.status.has_run() {
            return Ok(Some(ServerMessage::Cancel));
        }

        // The job's executions have run past its timeout
        if job_executions.is_past_deadline(now, grace_period)
            || execution.is_past_deadline(now, grace_period)
        {
            slog::warn!(log, "Job timeout exceeded during heartbeat"; "job_id" => ?job.id, "used" => job_executions.used_secs(now), "timeout" => u32::from(job.timeout));
            if end_job(conn, job.id, JobStatus::Canceled, now)? {
                context.callbacks.fire(log, job.id);
                #[cfg(feature = "otel")]
                bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerJobUpdate(
                    bencher_otel::JobStatusKind::Canceled,
                ));
            }
            return Ok(Some(ServerMessage::Cancel));
        }

        UpdateJobExecution::heartbeat(now).execute_if_any_status(
            conn,
            execution_id,
            &ExecutionStatus::IN_FLIGHT,
        )?;

        let billing = BillingDelta::new(job, job_executions.used_secs(now));
        let update = match &billing {
            Some(b) => UpdateJob::heartbeat_with_billing(now, b.minutes),
            None => UpdateJob::heartbeat(now),
        };
        update.execute_if_any_status(
            conn,
            job.id,
            &[JobStatus::Claimed, JobStatus::Running, JobStatus::Unknown],
        )?;

        if let Err(e) = QueryRunner::record_heartbeat(conn, execution.runner_id, now) {
            slog::warn!(log, "Failed to record runner heartbeat"; "error" => %e);
        }

        billing.map(|billing| (job.id, billing))
    };
    // write lock released here

    // Bill Stripe best-effort: the delta is already claimed in the DB.
    if let Some((job_id, billing)) = bill_info {
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
    /// Calculate the billing delta between the minutes of the job's executions, `elapsed_secs`,
    /// and the last billed minute.
    /// Returns `None` if the job hasn't started or no new minutes need billing.
    fn new(job: &QueryJob, elapsed_secs: i64) -> Option<Self> {
        job.started?;
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
                    let billing = biller.get_metered_plan_billing(&metered_plan_id).await?;
                    if billing.status.is_active() {
                        self.customer = CachedCustomer::Some(billing.customer_id.clone());
                        Ok(Some(billing.customer_id))
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

/// Final billing call when an execution ends.
///
/// Re-reads the job and its executions from the database to get the latest
/// `last_billed_minute` (which may have been updated by prior heartbeats or another
/// execution's end), bills any remaining partial-minute delta of the executions' summed
/// seconds, and persists the new `last_billed_minute`. Executions that end close together
/// bill once, since each reads the minute the other wrote.
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
        let job_executions = JobExecutions::get(conn, job_id)?;

        let Some(billing) = BillingDelta::new(&job_executions.job, job_executions.used_secs(now))
        else {
            return Ok(());
        };

        let update_job = UpdateJob::final_billing(billing.minutes, now);
        // No status filter needed here (unlike `handle_heartbeat` which uses
        // `execute_if_any_status`): the write lock held since `write_conn!`
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

/// What a runner reports about an execution.
enum RunResult {
    Completed(Vec<JsonIterationOutput>),
    Failed {
        results: Vec<JsonIterationOutput>,
        error: String,
    },
    Canceled,
}

/// Handle a runner's result for an execution, from its channel or resent after a reconnect.
///
/// A run that completed has its output stored as the execution's own blob at once, and parsed,
/// and its job goes on: to its next execution, or to its outcome once every iteration has one. A
/// `Failed` fails its job. A completed run that failed fails its job too, unless the job allows
/// failure, in which case its iteration is a gap. Output that Bencher cannot read fails its job,
/// with or without `allow_failure`. A runner that acknowledges a cancel ends the execution, and
/// the job if it has no outcome yet. A result for an execution or job that already has its outcome
/// is acknowledged and discarded.
///
/// Uses status filters on the UPDATEs to avoid TOCTOU races.
async fn handle_result(
    log: &slog::Logger,
    context: &ApiContext,
    execution_id: JobExecutionId,
    result: RunResult,
) -> Result<(), ChannelError> {
    let now = context.clock.now();
    let grace_period = context.job_timeout_grace_period;
    let (execution, job_executions) = {
        let conn = auth_conn!(context);
        let execution = QueryJobExecution::get(conn, execution_id)?;
        let job_executions = JobExecutions::get(conn, execution.job_id)?;
        (execution, job_executions)
    };
    let job = &job_executions.job;

    if !execution.status.is_in_flight() || !is_in_progress(job.status) {
        let duplicate = match &result {
            RunResult::Completed(_) => execution.status == ExecutionStatus::Completed,
            RunResult::Failed { .. } => execution.status == ExecutionStatus::Failed,
            RunResult::Canceled => execution.status == ExecutionStatus::Canceled,
        };
        if duplicate {
            slog::debug!(log, "Execution already has this result (idempotent duplicate)"; "job_id" => ?job.id, "execution_id" => ?execution_id);
        } else {
            discard_late_result(log, job.id, job.status);
        }
        return Ok(());
    }

    if matches!(result, RunResult::Completed(_)) && execution.status == ExecutionStatus::Sent {
        return Err(ChannelError::InvalidStateTransition {
            job_id: job.id,
            execution_id,
            target: ExecutionStatus::Completed,
            current: execution.status,
        });
    }

    // A silent execution's result is taken until its budget and the grace period run out.
    if execution.status == ExecutionStatus::Unknown
        && (execution.is_past_deadline(now, grace_period)
            || job_executions.is_past_deadline(now, grace_period))
    {
        if end_job_now(context, job.id, JobStatus::Canceled).await? {
            context.callbacks.fire(log, job.id);
        }
        discard_late_result(log, job.id, JobStatus::Canceled);
        return Ok(());
    }

    // A `Failed` fails the job whatever `allow_failure` says, so a run that never started, ran out
    // of time, or lost its runner is never a gap.
    let (results, failure, fails_job) = match result {
        RunResult::Canceled => {
            return handle_canceled(log, context, execution_id, job.id).await;
        },
        RunResult::Completed(results) => {
            let failure = run_failure(&results);
            let fails_job = !job.config.allow_failure.unwrap_or_default();
            (results, failure, fails_job)
        },
        RunResult::Failed { results, error } => (results, Some(error), true),
    };

    // Stored before the execution has its outcome, so a restart that finds it finds its output.
    let output = JsonJobOutput {
        results,
        error: failure.clone(),
    };
    context
        .oci_storage()
        .job_output()
        .put_execution(job.config.project, execution.uuid, &output)
        .await?;

    let status = if failure.is_some() {
        ExecutionStatus::Failed
    } else {
        ExecutionStatus::Completed
    };
    let job_failure = if let Some(error) = failure.filter(|_| fails_job) {
        Some(JobFailure::Run(error))
    } else {
        // Parsed before the write lock, so a parse never holds it.
        let runs = execution_runs(job, &execution, status, output);
        job.output_parser(context)
            .await?
            .parse_execution(execution.iteration, runs)
            .err()
            .map(JobFailure::Unreadable)
    };
    // 1 transaction, so neither a claim nor a restart finds a failed execution whose job has an
    // iteration to send.
    let recorded = write_transaction!(context, |conn| {
        // Read after the store, under the lock, so a heartbeat of another execution of the job
        // never bills to a later time than this execution's end.
        let now = context.clock.now();
        record_result(conn, execution_id, job, status, job_failure, now)
    })?;

    match recorded {
        Recorded::Late => discard_late_result(log, job.id, job.status),
        Recorded::JobFailed(job_executions, error) => {
            finish_failed_job(log, context, &job_executions, error).await;
        },
        Recorded::JobEnded => {
            slog::info!(log, "Job already has its outcome"; "job_id" => ?job.id);
        },
        Recorded::Ended => settle_job(log, context, job.id).await?,
    }
    Ok(())
}

/// End an execution with `status`, and fail its job with `job_failure`, if any, or else bring the
/// job's status up to date.
fn record_result(
    conn: &mut bencher_schema::context::DbConnection,
    execution_id: JobExecutionId,
    job: &QueryJob,
    status: ExecutionStatus,
    job_failure: Option<JobFailure>,
    now: bencher_json::DateTime,
) -> diesel::QueryResult<Recorded> {
    let updated = UpdateJobExecution::end(status, now).execute_if_any_status(
        conn,
        execution_id,
        &ExecutionStatus::IN_FLIGHT,
    )?;
    Ok(if updated == 0 {
        Recorded::Late
    } else if let Some(job_failure) = job_failure {
        if end_job(conn, job.id, JobStatus::Failed, now)? {
            let error = match job_failure {
                JobFailure::Run(error) => error,
                JobFailure::Unreadable(error) => {
                    ParseRecord::FAILED.update(conn, job.report_id)?;
                    error
                },
            };
            Recorded::JobFailed(Box::new(JobExecutions::get(conn, job.id)?), error)
        } else {
            Recorded::JobEnded
        }
    } else {
        sync_job_status(conn, job.id, now)?;
        Recorded::Ended
    })
}

/// Why an execution's result fails its job.
enum JobFailure {
    /// The runner sent `Failed`, or a completed run failed without `allow_failure`.
    Run(String),
    /// Bencher cannot read the run's output, and its report records that its results did not
    /// parse.
    Unreadable(String),
}

/// What recording an execution's result did.
enum Recorded {
    /// The execution was no longer in flight, so its result is late.
    Late,
    /// The execution ended, and its job goes on or has its outcome.
    Ended,
    /// The execution failed its job, with this error.
    JobFailed(Box<JobExecutions>, String),
    /// The execution would have failed its job, which already had its outcome.
    JobEnded,
}

/// A runner that acknowledges a cancel ends its execution, and its job if the job has no outcome
/// yet.
async fn handle_canceled(
    log: &slog::Logger,
    context: &ApiContext,
    execution_id: JobExecutionId,
    job_id: JobId,
) -> Result<(), ChannelError> {
    let ended = write_transaction!(context, |conn| {
        let now = context.clock.now();
        UpdateJobExecution::end(ExecutionStatus::Canceled, now).execute_if_any_status(
            conn,
            execution_id,
            &ExecutionStatus::IN_FLIGHT,
        )?;
        end_job(conn, job_id, JobStatus::Canceled, now)
    })?;
    if ended {
        context.callbacks.fire(log, job_id);
        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerJobUpdate(
            bencher_otel::JobStatusKind::Canceled,
        ));
    }
    Ok(())
}

/// Why a run the runner reported as completed failed, if it did: it has no result, or it exited
/// non-zero, which a runner reports as completed under `allow_failure`.
fn run_failure(results: &[JsonIterationOutput]) -> Option<String> {
    if results.is_empty() {
        return Some("The runner reported no result for the run".to_owned());
    }
    results
        .iter()
        .find(|result| result.exit_code != 0)
        .map(|result| {
            format!(
                "Benchmark exited with non-zero exit code: {}",
                result.exit_code
            )
        })
}

/// A runner result for a job that already has an outcome is acknowledged,
/// so the runner stops resending it, and discarded.
fn discard_late_result(log: &slog::Logger, job_id: JobId, status: JobStatus) {
    slog::warn!(log, "Discarding late runner result"; "job_id" => ?job_id, "status" => %status);
    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerLateResultDiscarded);
}

// --- Job claiming ---

// Table aliases for correlated subqueries in the claim query.
// job_org is used to check if an org already has a running job (Free tier limit).
// job_ip is used to check if a source IP already has a running job (Unclaimed tier limit).
diesel::alias!(schema::job as job_org: JobOrg);
diesel::alias!(schema::job as job_ip: JobIp);

/// Poll interval for long-polling (1 second)
const POLL_INTERVAL: Duration = Duration::from_secs(1);

/// OCI registry token TTL: 10 minutes (enough for image pull, short enough to limit exposure)
const OCI_RUNNER_TOKEN_TTL: u32 = 600;

/// A job's iteration sent to a runner as 1 execution.
struct Claimed {
    job: QueryJob,
    execution: QueryJobExecution,
    message: JsonClaimedJob,
}

/// Attempt to claim the next execution of a job with tier-based concurrency limits.
///
/// A job is eligible while it still has an iteration to send, none of its executions is
/// `Unknown`, it has time left, and its spec is one of the runner's. Its iterations may go to
/// different runners, and at the same time.
///
/// Priority tiers:
/// - Plus: Unlimited concurrent jobs
/// - Free: 1 concurrent job per organization
/// - Unclaimed: 1 concurrent job per source IP
///
/// A job between its iterations is still in progress, so it holds its organization or address.
/// Among eligible jobs, the highest tier goes first, then the job whose last execution went out
/// longest ago, with a job that has none counting as the oldest, then the oldest job.
///
/// Returns `Ok(Some(claimed))` if an execution was claimed, `Ok(None)` if no eligible jobs available.
async fn try_claim_job(
    log: &slog::Logger,
    context: &ApiContext,
    runner_key: &RunnerAuth,
) -> Result<Option<Claimed>, HttpError> {
    let now = context.clock.now();
    // Jobs out of time are passed over; one with nothing in flight fails once the lock is released.
    let mut passed_over = Vec::new();
    let mut out_of_time = Vec::new();

    // Acquire write lock for the entire read-check-update to prevent TOCTOU races
    // where concurrent runners could bypass concurrency limits.
    // Scoped so the lock is released before doing non-DB work (OCI token generation).
    let claimed = {
        let conn = write_conn!(context);
        loop {
            let next_job =
                next_eligible_job(conn, runner_key.runner_id, &passed_over).map_err(|e| {
                    HttpError::for_internal_error(format!("Failed to query pending jobs: {e}"))
                })?;
            let Some(job) = next_job else {
                break None;
            };

            let executions = QueryJobExecution::of_job(conn, job.id)
                .map_err(resource_conflict_err!(Job, job))?;
            let job_executions = JobExecutions { job, executions };
            let budget = job_executions.budget_secs(now);
            let Some(budget) = u32::try_from(budget)
                .ok()
                .and_then(|budget| Timeout::try_from(budget).ok())
            else {
                passed_over.push(job_executions.job.id);
                if !job_executions.has_in_flight() {
                    out_of_time.push(job_executions.job.id);
                }
                continue;
            };
            let JobExecutions { job, executions } = job_executions;

            let execution = claim_execution(conn, &job, executions.len(), runner_key, budget, now)
                .map_err(resource_conflict_err!(Job, job))?;
            let json_spec = QuerySpec::get(conn, job.spec_id).map(QuerySpec::into_json)?;
            break Some((job, execution, json_spec));
        }
    };

    for job_id in out_of_time {
        if let Err(e) = settle_job(log, context, job_id).await {
            slog::error!(log, "Failed to end a job out of time"; "job_id" => ?job_id, "error" => %e);
        }
    }

    let Some((job, execution, json_spec)) = claimed else {
        return Ok(None);
    };
    let message = build_claimed_job(context, &job, &execution, runner_key, json_spec)?;
    Ok(Some(Claimed {
        job,
        execution,
        message,
    }))
}

/// The next job with an iteration for the runner, other than those `passed_over`.
fn next_eligible_job(
    conn: &mut bencher_schema::context::DbConnection,
    runner_id: RunnerId,
    passed_over: &[JobId],
) -> diesel::QueryResult<Option<QueryJob>> {
    use diesel::{
        NullableExpressionMethods as _,
        dsl::{max, sql},
    };
    use schema::job::dsl::{claimed, created, id, organization_id, priority, source_ip, status};

    // A job between its iterations is still Claimed or Running.
    let in_progress = status
        .eq(JobStatus::Pending)
        .or(status.eq(JobStatus::Claimed))
        .or(status.eq(JobStatus::Running));

    // Tier 1: Plus (priority >= 200) - no concurrency limit
    let tier_unlimited = priority.ge(Priority::Plus);

    // Tier 2: Free (priority 100-199) - one concurrent job per organization
    // Block a pending job if another job of the same org is Claimed or Running, and a job in
    // progress only if one of them was claimed before it, so the first claimed always goes on.
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
                .filter(job_org.field(organization_id).eq(organization_id))
                .filter(job_org.field(id).ne(id))
                .filter(
                    status.eq(JobStatus::Pending).or(job_org
                        .field(claimed)
                        .assume_not_null()
                        .lt(claimed.assume_not_null())
                        .or(job_org
                            .field(claimed)
                            .assume_not_null()
                            .eq(claimed.assume_not_null())
                            .and(job_org.field(id).lt(id)))),
                ),
        )));

    // Tier 3: Unclaimed (priority < 100) - one concurrent job per source IP
    // Block as for Free, by the jobs of the same source_ip.
    let tier_unclaimed_eligible = priority.lt(Priority::Free).and(not(exists(
        job_ip
            .filter(
                job_ip
                    .field(status)
                    .eq(JobStatus::Claimed)
                    .or(job_ip.field(status).eq(JobStatus::Running)),
            )
            .filter(job_ip.field(source_ip).eq(source_ip))
            .filter(job_ip.field(id).ne(id))
            .filter(
                status.eq(JobStatus::Pending).or(job_ip
                    .field(claimed)
                    .assume_not_null()
                    .lt(claimed.assume_not_null())
                    .or(job_ip
                        .field(claimed)
                        .assume_not_null()
                        .eq(claimed.assume_not_null())
                        .and(job_ip.field(id).lt(id)))),
            ),
    )));

    // Combined eligibility: any tier condition passes
    let eligible = tier_unlimited
        .or(tier_free_eligible)
        .or(tier_unclaimed_eligible);

    // Spec filter: only claim jobs whose spec_id matches one of the runner's specs
    let spec_filter = schema::job::spec_id.eq_any(
        schema::runner_spec::table
            .filter(schema::runner_spec::runner_id.eq(runner_id))
            .select(schema::runner_spec::spec_id),
    );

    // The job still has an iteration to send: fewer executions than `iter`, at least 1.
    let has_iteration_left = schema::job_execution::table
        .filter(schema::job_execution::job_id.eq(id))
        .count()
        .single_value()
        .assume_not_null()
        .lt(sql::<diesel::sql_types::BigInt>(
            "MAX(COALESCE(json_extract(job.config, '$.iter'), 1), 1)",
        ));

    // Nothing more is sent for a job while one of its executions is silent.
    let none_unknown = not(exists(
        schema::job_execution::table
            .filter(schema::job_execution::job_id.eq(id))
            .filter(schema::job_execution::status.eq(ExecutionStatus::Unknown)),
    ));

    // Nothing more is sent for a job that an older server sent whole, under its own UUID.
    let not_sent_whole = not(exists(
        schema::job_execution::table
            .filter(schema::job_execution::job_id.eq(id))
            .filter(schema::job_execution::uuid.eq(schema::job::uuid)),
    ));

    // When the job's last execution went out; a job with none sorts first.
    let last_sent = schema::job_execution::table
        .filter(schema::job_execution::job_id.eq(id))
        .select(max(schema::job_execution::sent))
        .single_value();

    schema::job::table
        .filter(in_progress)
        .filter(eligible)
        .filter(spec_filter)
        .filter(has_iteration_left)
        .filter(none_unknown)
        .filter(not_sent_whole)
        .filter(id.ne_all(passed_over))
        .order((priority.desc(), last_sent.asc(), created.asc(), id.asc()))
        .first(conn)
        .optional()
}

/// Record the next iteration of `job` as an execution sent to the runner, and the runner as the
/// job's latest. The job's first execution claims it.
fn claim_execution(
    conn: &mut bencher_schema::context::DbConnection,
    job: &QueryJob,
    iteration: usize,
    runner_key: &RunnerAuth,
    budget: Timeout,
    now: bencher_json::DateTime,
) -> diesel::QueryResult<QueryJobExecution> {
    let insert = InsertJobExecution {
        uuid: JobUuid::new(),
        job_id: job.id,
        report_id: job.report_id,
        iteration: i32::try_from(iteration).unwrap_or(i32::MAX),
        runner_id: runner_key.runner_id,
        timeout: budget,
        status: ExecutionStatus::Sent,
        sent: now,
        started: None,
        last_heartbeat: now,
    };
    diesel::insert_into(schema::job_execution::table)
        .values(&insert)
        .execute(conn)?;
    if job.status == JobStatus::Pending {
        UpdateJob::claim(runner_key.runner_id, now).execute_if_status(
            conn,
            job.id,
            JobStatus::Pending,
        )?;
    } else {
        UpdateJob {
            runner_id: Some(Some(runner_key.runner_id)),
            modified: Some(now),
            ..Default::default()
        }
        .execute(conn, job.id)?;
    }
    schema::job_execution::table
        .filter(schema::job_execution::uuid.eq(insert.uuid))
        .first(conn)
}

/// The `Job` message for 1 execution: the job's config as 1 iteration, with what is left of the
/// job's timeout as its timeout, under the execution's UUID, and with a pull token of its own.
fn build_claimed_job(
    context: &ApiContext,
    job: &QueryJob,
    execution: &QueryJobExecution,
    runner_key: &RunnerAuth,
    json_spec: JsonSpec,
) -> Result<JsonClaimedJob, HttpError> {
    #[cfg(feature = "otel")]
    {
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerJobClaim);

        // Record queue duration (time from creation to claim)
        let now = context.clock.now();
        let queue_duration_secs = job.created.elapsed_secs(now);
        bencher_otel::ApiMeter::record(
            bencher_otel::ApiHistogram::JobQueueDuration(job.priority),
            queue_duration_secs,
        );
    }

    let oci_token = context
        .token_key
        .new_oci_runner(
            runner_key.runner_uuid,
            OCI_RUNNER_TOKEN_TTL,
            Some(job.config.project.to_string()),
            vec![OciAction::Pull],
        )
        .map_err(|e| {
            HttpError::for_internal_error(format!("Failed to generate OCI runner token: {e}"))
        })?;

    let mut config = job.config.clone();
    config.iter = Some(Iteration(1));
    config.timeout = execution.timeout;
    Ok(JsonClaimedJob {
        uuid: execution.uuid,
        spec: json_spec,
        config,
        oci_token,
        timeout: execution.timeout,
        created: job.created,
    })
}

// --- Channel endpoint ---

/// Result of the executing state loop.
enum ExecuteResult {
    /// Job reached a terminal state (Completed/Failed/Canceled).
    JobDone,
    /// Runner disconnected or heartbeat timed out.
    Disconnected,
    /// The server is shutting down; close the channel so `server.close()` can complete.
    ShuttingDown,
}

/// Spawn a heartbeat timeout for an execution whose channel ended (client disconnect, execute
/// error, or server shutdown) before it finished, so the execution in flight is not orphaned: the
/// runner can reconnect to another instance and resume within the grace period. `reason` is
/// logged for context.
async fn spawn_inflight_heartbeat_timeout(
    log: &slog::Logger,
    context: &ApiContext,
    heartbeat_timeout: Duration,
    execution_id: JobExecutionId,
    reason: &str,
) -> Result<(), ChannelError> {
    let execution = QueryJobExecution::get(auth_conn!(context), execution_id)?;
    if execution.status.is_in_flight() {
        slog::info!(log, "Channel ended ({reason}) for in-flight execution, spawning heartbeat timeout"; "job_id" => ?execution.job_id, "execution_id" => ?execution.id);
        JobTimeout {
            heartbeat: heartbeat_timeout,
            grace_period: context.job_timeout_grace_period,
        }
        .spawn(
            log.clone(),
            context.database.connection.clone(),
            execution.id,
            &context.heartbeat_tasks,
            context.clock.clone(),
            context.callbacks.clone(),
        );
    }
    Ok(())
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
/// or `NoJob`. A runner that sends `Paused` instead gets `NoJob` at the end of
/// its poll and no job. During execution, handles `Running`, `Heartbeat`,
/// `Completed`, `Failed`, and `Canceled` messages.
/// Authentication is via runner key in the Authorization header.
#[channel {
    protocol = WEBSOCKETS,
    path = "/v0/runners/{runner}/channel",
    tags = ["runners"]
}]
#[expect(
    clippy::too_many_lines,
    reason = "WebSocket handler with auth, rate limiting, and message loop"
)]
pub async fn runner_channel(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<RunnerChannelParams>,
    conn: WebsocketConnection,
) -> WebsocketChannelResult {
    let context = rqctx.context();
    let log = rqctx.log.clone();
    let path_params = path_params.into_inner();

    // Validate runner key from Authorization header
    let runner_key = RunnerAuth::from_request(&rqctx, &path_params.runner).await?;

    // Per-runner rate limiting
    #[cfg(feature = "plus")]
    context
        .rate_limiting
        .runner_request(runner_key.runner_uuid)?;

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
    let shutdown = &context.shutdown;

    #[cfg(feature = "otel")]
    let mut state_guard = RunnerStateGuard::new(bencher_otel::RunnerStateKind::Idle);

    // Set when graceful shutdown is signalled so we send a Close frame after leaving the loop.
    let mut shutting_down = false;
    let mut paused = false;

    // State machine: Idle -> Executing -> Idle -> ...
    loop {
        // === IDLE STATE ===
        // Wait for Ready or Paused from runner (also handles terminal message retries)
        let (poll_timeout, claim) = match wait_for_ready(
            &log,
            context,
            runner_key.runner_id,
            &mut tx,
            &mut rx,
            heartbeat_timeout,
            shutdown,
        )
        .await
        {
            Ok(ReadyOutcome::Ready(poll_timeout)) => {
                if paused {
                    paused = false;
                    slog::info!(log, "Runner pause ended");
                    #[cfg(feature = "otel")]
                    state_guard.transition(bencher_otel::RunnerStateKind::Idle);
                }
                (poll_timeout, true)
            },
            Ok(ReadyOutcome::Paused(pause)) => {
                if !paused {
                    paused = true;
                    slog::info!(log, "Runner pause began"; "reasons" => ?pause.reasons, "since" => %pause.since);
                    #[cfg(feature = "otel")]
                    {
                        state_guard.transition(bencher_otel::RunnerStateKind::Paused);
                        count_pause(&pause.reasons);
                    }
                }
                (poll_timeout_secs(pause.ready.poll_timeout), false)
            },
            Ok(ReadyOutcome::UpdateSent) => {
                #[cfg(feature = "otel")]
                state_guard.transition(bencher_otel::RunnerStateKind::Updating);
                break;
            },
            Ok(ReadyOutcome::ShuttingDown) => {
                shutting_down = true;
                break;
            },
            Err(_) => {
                #[cfg(feature = "otel")]
                bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerDisconnect);
                break;
            },
        };

        let deadline = tokio::time::Instant::now() + Duration::from_secs(u64::from(poll_timeout));

        // Poll for a job, or hold a paused runner's poll, checking for WS disconnect between polls
        let claimed_job = if claim {
            poll_for_job(
                &log,
                context,
                &runner_key,
                deadline,
                &mut tx,
                &mut rx,
                shutdown,
            )
            .await
        } else {
            hold_for_poll(&log, deadline, &mut tx, &mut rx, shutdown).await
        };

        match claimed_job {
            Ok(PollOutcome::Claimed(claimed)) => {
                let Claimed {
                    job: query_job,
                    execution,
                    message,
                } = *claimed;
                // Send Job to runner
                let job_msg = ServerMessage::Job(Box::new(message));
                let text = serde_json::to_string(&job_msg)
                    .map_err(|e| HttpError::for_internal_error(e.to_string()))?;
                if tx.send(Message::Text(text.into())).await.is_err() {
                    #[cfg(feature = "otel")]
                    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerDisconnect);
                    break;
                }

                // === EXECUTING STATE ===
                #[cfg(feature = "otel")]
                state_guard.transition(bencher_otel::RunnerStateKind::Executing);

                match execute_loop(
                    &log,
                    context,
                    &execution,
                    &mut tx,
                    &mut rx,
                    heartbeat_timeout,
                    shutdown,
                )
                .await
                {
                    Ok(ExecuteResult::JobDone) => {
                        #[cfg(feature = "otel")]
                        state_guard.transition(bencher_otel::RunnerStateKind::Idle);
                    },
                    Ok(ExecuteResult::Disconnected) => {
                        #[cfg(feature = "otel")]
                        bencher_otel::ApiMeter::increment(
                            bencher_otel::ApiCounter::RunnerDisconnect,
                        );
                        spawn_inflight_heartbeat_timeout(
                            &log,
                            context,
                            heartbeat_timeout,
                            execution.id,
                            "client disconnect",
                        )
                        .await?;
                        break;
                    },
                    Ok(ExecuteResult::ShuttingDown) => {
                        // Server shutting down mid-job: leave the job Running and close the channel.
                        // Unlike the disconnect branch we do NOT spawn a heartbeat timeout here: the
                        // process is exiting, so the runtime would abort that task before it could
                        // fire. Recovery is resume-on-another-instance: the runner reconnects and
                        // re-sends its terminal result (`wait_for_ready` handles the retry), with
                        // billing continuing from the persisted `last_billed_minute` watermark.
                        shutting_down = true;
                        break;
                    },
                    Err(e) => {
                        #[cfg(feature = "otel")]
                        bencher_otel::ApiMeter::increment(
                            bencher_otel::ApiCounter::RunnerDisconnect,
                        );
                        slog::error!(log, "Execute loop error"; "error" => %e, "job_id" => ?query_job.id);
                        spawn_inflight_heartbeat_timeout(
                            &log,
                            context,
                            heartbeat_timeout,
                            execution.id,
                            "execute error",
                        )
                        .await?;
                        break;
                    },
                }
            },
            Ok(PollOutcome::Deadline) => {
                // No job available, send NoJob and stay in Idle
                let text = serde_json::to_string(&ServerMessage::NoJob)
                    .map_err(|e| HttpError::for_internal_error(e.to_string()))?;
                if tx.send(Message::Text(text.into())).await.is_err() {
                    #[cfg(feature = "otel")]
                    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerDisconnect);
                    break;
                }
            },
            Ok(PollOutcome::ShuttingDown) => {
                shutting_down = true;
                break;
            },
            Err(_) => {
                #[cfg(feature = "otel")]
                bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerDisconnect);
                break;
            },
        }
    }

    // Tell the runner we're going away so it reconnects to the next instance, then return so
    // `server.close()` can complete instead of hanging on this persistent connection.
    if shutting_down {
        slog::info!(log, "Closing runner channel for server shutdown");
        let close_frame = CloseFrame {
            code: CloseCode::Away,
            reason: "server shutting down".into(),
        };
        drop(tx.send(Message::Close(Some(close_frame))).await);
    }

    Ok(())
}

/// Outcome of waiting for a `RunnerMessage::Ready` or `RunnerMessage::Paused` message.
enum ReadyOutcome {
    /// Runner version matches; proceed with job polling.
    Ready(u32),
    /// Runner version matches but the runner is paused; hold its poll without claiming.
    Paused(Box<JsonPaused>),
    /// Runner version is outdated; `Update` message was sent.
    UpdateSent,
    /// The server is shutting down; close the channel so `server.close()` can complete.
    ShuttingDown,
}

#[cfg(feature = "otel")]
struct RunnerStateGuard {
    state: bencher_otel::RunnerStateKind,
}

#[cfg(feature = "otel")]
impl RunnerStateGuard {
    fn new(state: bencher_otel::RunnerStateKind) -> Self {
        bencher_otel::ApiMeter::up(bencher_otel::ApiGauge::RunnerState(state));
        Self { state }
    }

    fn transition(&mut self, new_state: bencher_otel::RunnerStateKind) {
        bencher_otel::ApiMeter::down(bencher_otel::ApiGauge::RunnerState(self.state));
        self.state = new_state;
        bencher_otel::ApiMeter::up(bencher_otel::ApiGauge::RunnerState(self.state));
    }
}

#[cfg(feature = "otel")]
impl Drop for RunnerStateGuard {
    fn drop(&mut self) {
        bencher_otel::ApiMeter::down(bencher_otel::ApiGauge::RunnerState(self.state));
    }
}

/// Count a pause that began, once per reason kind.
#[cfg(feature = "otel")]
fn count_pause(reasons: &[PauseReason]) {
    let mut kinds = Vec::with_capacity(reasons.len());
    for reason in reasons {
        let kind = match reason {
            PauseReason::Raid { .. } => bencher_otel::PauseReasonKind::Raid,
            PauseReason::Maintenance { .. } => bencher_otel::PauseReasonKind::Maintenance,
            PauseReason::Other => bencher_otel::PauseReasonKind::Other,
        };
        if !kinds.contains(&kind) {
            kinds.push(kind);
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerPause(kind));
        }
    }
}

const MAX_CHECKSUM_RESPONSE_BYTES: usize = 1024;

/// Fetch the SHA-256 checksum for a runner binary from its companion `.sha256` file.
async fn fetch_runner_checksum(binary_url: url::Url) -> Result<bencher_json::Sha256, ChannelError> {
    let checksum_url = format!("{binary_url}.sha256");
    let response = reqwest::get(&checksum_url)
        .await
        .map_err(ChannelError::UpdateHttp)?;
    if !response.status().is_success() {
        return Err(ChannelError::UpdateHttpStatus(response.status()));
    }
    let bytes = response.bytes().await.map_err(ChannelError::UpdateHttp)?;
    if bytes.len() > MAX_CHECKSUM_RESPONSE_BYTES {
        return Err(ChannelError::UpdateChecksumTooLarge(bytes.len()));
    }
    let body = std::str::from_utf8(&bytes).map_err(ChannelError::UpdateChecksumUtf8)?;
    let hex = body
        .split_whitespace()
        .next()
        .ok_or(ChannelError::UpdateEmptyChecksum)?;
    hex.parse().map_err(ChannelError::UpdateChecksum)
}

/// Wait for a `RunnerMessage::Ready` or `RunnerMessage::Paused` message, returning the poll timeout.
///
/// If the runner's version does not match the server's version, sends a
/// `ServerMessage::Update` with the download URL and checksum, then returns
/// `ReadyOutcome::UpdateSent`.
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
    shutdown: &CancellationToken,
) -> Result<ReadyOutcome, ChannelError>
where
    S: futures::Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
    R: futures::Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    loop {
        let msg = match shutdown
            .run_until_cancelled(tokio::time::timeout(idle_timeout, rx.next()))
            .await
        {
            None => return Ok(ReadyOutcome::ShuttingDown),
            Some(Ok(Some(msg))) => msg,
            Some(Ok(None)) => {
                return Err(ChannelError::WebSocket(
                    tokio_tungstenite::tungstenite::Error::ConnectionClosed,
                ));
            },
            Some(Err(_elapsed)) => {
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
                    RunnerMessage::Ready(ready) => {
                        record_status(log, context, runner_id, RunnerReport::ready(&ready)).await;
                        // Boxed to keep the long-lived `runner_channel` future small;
                        // this runs once per poll cycle, so the allocation is cheap.
                        return Box::pin(handle_ready(
                            log,
                            &context.runner_update,
                            &context.clock,
                            tx,
                            ready.poll_timeout,
                            ready.runner.as_ref(),
                            fetch_runner_checksum,
                        ))
                        .await;
                    },
                    RunnerMessage::Paused(paused) => {
                        record_status(log, context, runner_id, RunnerReport::paused(&paused)).await;
                        return Box::pin(handle_paused(
                            log,
                            &context.runner_update,
                            &context.clock,
                            tx,
                            paused,
                            fetch_runner_checksum,
                        ))
                        .await;
                    },
                    terminal_msg @ (RunnerMessage::Completed { .. }
                    | RunnerMessage::Failed { .. }
                    | RunnerMessage::Canceled { .. }) => {
                        // Boxed to keep the long-lived `runner_channel` future small;
                        // reconnect retries are rare, so the allocation is cheap.
                        Box::pin(handle_idle_terminal(
                            log,
                            context,
                            runner_id,
                            tx,
                            terminal_msg,
                        ))
                        .await?;
                    },
                    RunnerMessage::Running | RunnerMessage::Heartbeat => {
                        slog::warn!(log, "Unexpected message in Idle state, expected Ready"; "runner_msg" => ?runner_msg);
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

/// Note that the runner is alive, store its status if it changed, and mail admins about news,
/// without failing the channel.
async fn record_status(
    log: &slog::Logger,
    context: &ApiContext,
    runner_id: RunnerId,
    report: RunnerReport,
) {
    let now = context.clock.now();
    let change = {
        let conn = write_conn!(context);
        if let Err(e) = QueryRunner::record_heartbeat(conn, runner_id, now) {
            slog::warn!(log, "Failed to record runner heartbeat"; "error" => %e);
        }
        QueryRunnerStatus::record(conn, runner_id, report, now)
    };
    match change {
        Ok(Some(change)) => {
            slog::info!(log, "Runner status changed"; "availability" => ?change.new.availability, "health" => ?change.new.health.as_ref().map(|health| &health.0.state));
            if let Err(e) = notify_admins(log, context, runner_id, &change).await {
                slog::warn!(log, "Failed to mail admins about a runner status change"; "error" => %e);
            }
        },
        Ok(None) => {},
        Err(e) => slog::warn!(log, "Failed to record runner status"; "error" => %e),
    }
}

async fn notify_admins(
    log: &slog::Logger,
    context: &ApiContext,
    runner_id: RunnerId,
    change: &StatusChange,
) -> Result<(), HttpError> {
    let conn = auth_conn!(context);
    let runner = QueryRunner::get(conn, runner_id)?;
    change.notify(log, conn, &context.messenger, runner.name.as_ref())
}

/// Handle a terminal message (Completed/Failed/Canceled) received during Idle,
/// which occurs when a runner reconnects with a pending result that was not acknowledged.
/// The message names its execution, which lands the result on its own iteration even after the
/// job's next execution went out, to this runner or another.
async fn handle_idle_terminal<S>(
    log: &slog::Logger,
    context: &ApiContext,
    runner_id: RunnerId,
    tx: &mut S,
    terminal_msg: RunnerMessage,
) -> Result<(), ChannelError>
where
    S: futures::Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    let (execution_uuid, result) = match terminal_msg {
        RunnerMessage::Completed {
            job: execution_uuid,
            results,
        } => {
            slog::info!(log, "Received Completed during Idle (retry after reconnect)"; "job_uuid" => %execution_uuid);
            (execution_uuid, RunResult::Completed(results))
        },
        RunnerMessage::Failed {
            job: execution_uuid,
            results,
            error,
        } => {
            slog::info!(log, "Received Failed during Idle (retry after reconnect)"; "job_uuid" => %execution_uuid, "error" => &error);
            (execution_uuid, RunResult::Failed { results, error })
        },
        RunnerMessage::Canceled {
            job: execution_uuid,
        } => {
            slog::info!(log, "Received Canceled during Idle (retry after reconnect)"; "job_uuid" => %execution_uuid);
            (execution_uuid, RunResult::Canceled)
        },
        // Non-terminal messages are never passed in by the caller.
        RunnerMessage::Ready(_)
        | RunnerMessage::Paused(_)
        | RunnerMessage::Running
        | RunnerMessage::Heartbeat => {
            return Ok(());
        },
    };

    if let Some(execution) =
        lookup_runner_execution(log, context, runner_id, execution_uuid).await?
    {
        handle_result(log, context, execution.id, result).await?;
        bill_final_minutes(log, context, execution.job_id, &mut BillingState::new()).await;
    }

    let ack = serde_json::to_string(&ServerMessage::Ack {
        job: Some(execution_uuid),
    })?;
    tx.send(Message::Text(ack.into())).await?;
    Ok(())
}

/// The display version sent with canary channel updates; convergence is checksum-based.
const CANARY_UPDATE_VERSION: &str = "canary";

/// Check whether the runner should self-update, per its update channel.
///
/// If runner metadata is absent (or the runner is not on Linux), skips auto-update
/// and returns `ReadyOutcome::Ready` with the poll timeout.
/// On the stable channel, the runner is up to date when its version matches the
/// server version. On the canary channel, the runner is up to date when its
/// self-reported binary checksum matches the published canary channel checksum.
/// If an update is due, sends `ServerMessage::Update` with the download URL and
/// checksum, then returns `ReadyOutcome::UpdateSent`.
///
/// A failure to fetch the published checksum skips the update (with a warning
/// and an adverse event counter) rather than erroring the channel: the runner
/// stays on its current version and the check retries at its next `Ready`.
async fn handle_ready<S, F, Fut>(
    log: &slog::Logger,
    runner_update: &bencher_schema::context::RunnerUpdate,
    clock: &bencher_json::Clock,
    tx: &mut S,
    poll_timeout: Option<bencher_json::PollTimeout>,
    runner: Option<&bencher_json::runner::JsonRunnerMetadata>,
    fetch_checksum: F,
) -> Result<ReadyOutcome, ChannelError>
where
    S: futures::Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
    F: Fn(url::Url) -> Fut,
    Fut: Future<Output = Result<bencher_json::Sha256, ChannelError>>,
{
    let timeout = poll_timeout_secs(poll_timeout);

    let Some(runner) = runner else {
        slog::info!(log, "Runner ready (no metadata, skipping auto-update)");
        return Ok(ReadyOutcome::Ready(timeout));
    };

    slog::info!(log, "Runner ready"; "os" => %runner.os, "arch" => %runner.arch, "version" => &runner.version, "channel" => %runner.channel.unwrap_or_default(), "server_version" => bencher_json::BENCHER_API_VERSION);

    Ok(
        if send_update(log, runner_update, clock, tx, runner, &fetch_checksum).await? {
            ReadyOutcome::UpdateSent
        } else {
            ReadyOutcome::Ready(timeout)
        },
    )
}

/// Run the same update check as `handle_ready` for a paused runner, which then gets no job.
async fn handle_paused<S, F, Fut>(
    log: &slog::Logger,
    runner_update: &bencher_schema::context::RunnerUpdate,
    clock: &bencher_json::Clock,
    tx: &mut S,
    mut paused: JsonPaused,
    fetch_checksum: F,
) -> Result<ReadyOutcome, ChannelError>
where
    S: futures::Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
    F: Fn(url::Url) -> Fut,
    Fut: Future<Output = Result<bencher_json::Sha256, ChannelError>>,
{
    paused.reasons.truncate(MAX_PAUSE_REASONS);
    for reason in &mut paused.reasons {
        reason.cap();
    }
    slog::debug!(log, "Runner paused"; "reasons" => ?paused.reasons, "since" => %paused.since);

    if let Some(runner) = &paused.ready.runner
        && send_update(log, runner_update, clock, tx, runner, &fetch_checksum).await?
    {
        return Ok(ReadyOutcome::UpdateSent);
    }
    Ok(ReadyOutcome::Paused(Box::new(paused)))
}

fn poll_timeout_secs(poll_timeout: Option<bencher_json::PollTimeout>) -> u32 {
    poll_timeout.map_or(DEFAULT_POLL_TIMEOUT, u32::from)
}

/// Send `ServerMessage::Update` if the runner is due one, and return whether it was sent.
async fn send_update<S, F, Fut>(
    log: &slog::Logger,
    runner_update: &bencher_schema::context::RunnerUpdate,
    clock: &bencher_json::Clock,
    tx: &mut S,
    runner: &bencher_json::runner::JsonRunnerMetadata,
    fetch_checksum: &F,
) -> Result<bool, ChannelError>
where
    S: futures::Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
    F: Fn(url::Url) -> Fut,
    Fut: Future<Output = Result<bencher_json::Sha256, ChannelError>>,
{
    if runner.os != bencher_json::OperatingSystem::Linux {
        return Ok(false);
    }

    let channel = runner.channel.unwrap_or_default();
    let update = match channel {
        bencher_json::UpdateChannel::Stable => {
            stable_update(
                log,
                runner_update,
                runner,
                bencher_json::BENCHER_API_VERSION,
                fetch_checksum,
            )
            .await?
        },
        bencher_json::UpdateChannel::Canary => {
            canary_update(log, runner_update, clock, runner, fetch_checksum).await?
        },
    };

    let Some(update) = update else {
        return Ok(false);
    };

    let text = serde_json::to_string(&update)?;
    tx.send(Message::Text(text.into())).await?;

    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerSelfUpdateSent(
        update_channel_kind(channel),
    ));

    Ok(true)
}

/// Stable channel: update when the runner version differs from the server version,
/// targeting the versioned GitHub Release.
///
/// The published checksum is deliberately not cached (unlike the canary channel):
/// it is only fetched while a stale runner has not yet updated, which is rare
/// and self-limiting.
async fn stable_update<F, Fut>(
    log: &slog::Logger,
    runner_update: &bencher_schema::context::RunnerUpdate,
    runner: &bencher_json::runner::JsonRunnerMetadata,
    server_version: &str,
    fetch_checksum: &F,
) -> Result<Option<ServerMessage>, ChannelError>
where
    F: Fn(url::Url) -> Fut,
    Fut: Future<Output = Result<bencher_json::Sha256, ChannelError>>,
{
    if runner.version == server_version {
        return Ok(None);
    }

    let url = runner_update
        .stable_url(server_version, runner.arch)
        .map_err(ChannelError::UpdateUrl)?;
    let checksum = match fetch_checksum(url.clone()).await {
        Ok(checksum) => checksum,
        Err(e) => {
            slog::warn!(log, "Failed to fetch stable runner checksum, skipping update"; "url" => %url, "error" => %e);
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(
                bencher_otel::ApiCounter::RunnerSelfUpdateCheckFailed(
                    bencher_otel::UpdateChannelKind::Stable,
                ),
            );
            return Ok(None);
        },
    };

    slog::info!(log, "Runner version mismatch, sending update"; "runner" => &runner.version, "server" => server_version);
    Ok(Some(ServerMessage::Update {
        version: server_version.to_owned(),
        url,
        checksum,
    }))
}

/// Canary channel: update when the runner's self-reported binary checksum differs
/// from the published rolling canary channel checksum (TTL-cached per architecture).
async fn canary_update<F, Fut>(
    log: &slog::Logger,
    runner_update: &bencher_schema::context::RunnerUpdate,
    clock: &bencher_json::Clock,
    runner: &bencher_json::runner::JsonRunnerMetadata,
    fetch_checksum: &F,
) -> Result<Option<ServerMessage>, ChannelError>
where
    F: Fn(url::Url) -> Fut,
    Fut: Future<Output = Result<bencher_json::Sha256, ChannelError>>,
{
    let Some(runner_checksum) = &runner.checksum else {
        slog::warn!(
            log,
            "Canary channel runner reported no checksum, skipping auto-update"
        );
        return Ok(None);
    };

    let url = runner_update
        .canary_url(runner.arch)
        .map_err(ChannelError::UpdateUrl)?;

    let published = if let Some(cached) = runner_update.cached_canary_checksum(clock, runner.arch) {
        cached
    } else {
        match fetch_checksum(url.clone()).await {
            Ok(checksum) => {
                runner_update.store_canary_checksum(clock, runner.arch, checksum.clone());
                checksum
            },
            Err(e) => {
                slog::warn!(log, "Failed to fetch canary runner checksum, skipping update"; "url" => %url, "error" => %e);
                #[cfg(feature = "otel")]
                bencher_otel::ApiMeter::increment(
                    bencher_otel::ApiCounter::RunnerSelfUpdateCheckFailed(
                        bencher_otel::UpdateChannelKind::Canary,
                    ),
                );
                return Ok(None);
            },
        }
    };

    if published == *runner_checksum {
        return Ok(None);
    }

    slog::info!(log, "Canary channel runner checksum mismatch, sending update"; "runner" => %runner_checksum, "published" => %published);
    Ok(Some(ServerMessage::Update {
        version: CANARY_UPDATE_VERSION.to_owned(),
        url,
        checksum: published,
    }))
}

#[cfg(feature = "otel")]
fn update_channel_kind(channel: bencher_json::UpdateChannel) -> bencher_otel::UpdateChannelKind {
    match channel {
        bencher_json::UpdateChannel::Stable => bencher_otel::UpdateChannelKind::Stable,
        bencher_json::UpdateChannel::Canary => bencher_otel::UpdateChannelKind::Canary,
    }
}

/// Look up an execution by UUID and verify it was sent to the given runner.
///
/// Returns `Ok(Some(execution))` if found and sent to this runner,
/// `Ok(None)` if not found or sent to a different runner (with a warning log).
async fn lookup_runner_execution(
    log: &slog::Logger,
    context: &ApiContext,
    runner_id: RunnerId,
    execution_uuid: JobUuid,
) -> Result<Option<QueryJobExecution>, ChannelError> {
    let Some(execution) = QueryJobExecution::from_uuid(auth_conn!(context), execution_uuid)? else {
        slog::warn!(log, "Terminal message for unknown job during Idle"; "job_uuid" => %execution_uuid);
        return Ok(None);
    };

    if execution.runner_id != runner_id {
        slog::warn!(log, "Terminal message for job not owned by this runner"; "job_uuid" => %execution_uuid);
        return Ok(None);
    }

    Ok(Some(execution))
}

/// Outcome of polling for a job during the idle state.
enum PollOutcome {
    /// An execution of a job was claimed for this runner.
    Claimed(Box<Claimed>),
    /// The poll deadline elapsed without claiming a job.
    Deadline,
    /// The server is shutting down; close the channel so `server.close()` can complete.
    ShuttingDown,
}

/// Poll for a job, checking for WS disconnect between polls.
///
/// Returns `Ok(PollOutcome::Claimed)` if claimed, `Ok(PollOutcome::Deadline)` if the deadline
/// expired, `Ok(PollOutcome::ShuttingDown)` if the server is shutting down, or `Err` on WS disconnect.
async fn poll_for_job<S, R>(
    log: &slog::Logger,
    context: &ApiContext,
    runner_key: &RunnerAuth,
    deadline: tokio::time::Instant,
    tx: &mut S,
    rx: &mut R,
    shutdown: &CancellationToken,
) -> Result<PollOutcome, ChannelError>
where
    S: futures::Sink<Message> + Unpin,
    R: futures::Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    poll_until(log, Some((context, runner_key)), deadline, tx, rx, shutdown).await
}

/// Hold a paused runner's poll until the deadline as `poll_for_job` would, but never claim a job.
async fn hold_for_poll<S, R>(
    log: &slog::Logger,
    deadline: tokio::time::Instant,
    tx: &mut S,
    rx: &mut R,
    shutdown: &CancellationToken,
) -> Result<PollOutcome, ChannelError>
where
    S: futures::Sink<Message> + Unpin,
    R: futures::Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    poll_until(log, None, deadline, tx, rx, shutdown).await
}

async fn poll_until<S, R>(
    log: &slog::Logger,
    claimant: Option<(&ApiContext, &RunnerAuth)>,
    deadline: tokio::time::Instant,
    tx: &mut S,
    rx: &mut R,
    shutdown: &CancellationToken,
) -> Result<PollOutcome, ChannelError>
where
    S: futures::Sink<Message> + Unpin,
    R: futures::Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    loop {
        if let Some((context, runner_key)) = claimant {
            match try_claim_job(log, context, runner_key).await {
                Ok(Some(claimed)) => {
                    return Ok(PollOutcome::Claimed(Box::new(claimed)));
                },
                Ok(None) => {},
                Err(e) => {
                    slog::error!(log, "Error claiming job"; "error" => %e);
                    // Continue polling: transient DB errors shouldn't break the channel
                },
            }
        }

        if tokio::time::Instant::now() >= deadline {
            return Ok(PollOutcome::Deadline);
        }

        // Wait POLL_INTERVAL, but check the WS for disconnect and the server for shutdown
        match shutdown
            .run_until_cancelled(tokio::time::timeout(POLL_INTERVAL, rx.next()))
            .await
        {
            None => return Ok(PollOutcome::ShuttingDown),
            Some(Ok(Some(Ok(Message::Close(_))) | None)) => {
                return Err(ChannelError::WebSocket(
                    tokio_tungstenite::tungstenite::Error::ConnectionClosed,
                ));
            },
            Some(Ok(Some(Ok(Message::Ping(data))))) => {
                if tx.send(Message::Pong(data)).await.is_err() {
                    slog::warn!(log, "Failed to send pong during polling");
                }
            },
            Some(Ok(Some(Err(e)))) => return Err(ChannelError::WebSocket(e)),
            Some(Ok(Some(Ok(Message::Text(text))))) => {
                slog::warn!(log, "Unexpected text message during polling"; "text" => %text);
            },
            Some(Ok(Some(Ok(Message::Binary(data))))) => {
                slog::warn!(log, "Unexpected binary message during polling"; "len" => data.len());
            },
            Some(Ok(Some(Ok(Message::Pong(_) | Message::Frame(_)))) | Err(_)) => {},
        }
    }
}

/// Run an execution on the channel: handle messages until terminal or disconnect.
///
/// Similar to the per-job WS handler but does not send close frames on terminal
/// messages: the connection stays open for the next execution.
async fn execute_loop<S, R>(
    log: &slog::Logger,
    context: &ApiContext,
    execution: &QueryJobExecution,
    tx: &mut S,
    rx: &mut R,
    heartbeat_timeout: Duration,
    shutdown: &CancellationToken,
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

        let msg_result = match shutdown
            .run_until_cancelled(tokio::time::timeout(remaining, rx.next()))
            .await
        {
            None => {
                // Server shutting down: return promptly. Unlike the disconnect and timeout branches,
                // we deliberately do NOT call `bill_final_minutes` here: it awaits a Stripe call, and
                // the whole point of this cancellation path is to drain `server.close()` fast (a slow
                // Stripe round-trip would risk the SIGKILL-before-save hang this fix prevents). The job
                // resumes on another instance, which re-bills the elapsed minutes from the persisted
                // `last_billed_minute` watermark (`BillingDelta` is job-age-based).
                return Ok(ExecuteResult::ShuttingDown);
            },
            Some(Ok(Some(msg_result))) => msg_result,
            Some(Ok(None)) => {
                // Stream ended (client disconnected) — best-effort bill any remaining partial minute
                bill_final_minutes(log, context, execution.job_id, &mut billing_state).await;
                return Ok(ExecuteResult::Disconnected);
            },
            Some(Err(_elapsed)) => {
                // Heartbeat timeout — best-effort bill any remaining partial minute before marking job
                bill_final_minutes(log, context, execution.job_id, &mut billing_state).await;
                let reason = handle_timeout(log, context, execution.id).await?;
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
                bill_final_minutes(log, context, execution.job_id, &mut billing_state).await;
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
                // spurious Ready or Paused messages: a misbehaving runner could send
                // them periodically to keep the heartbeat alive indefinitely.
                let is_idle = matches!(
                    runner_msg,
                    RunnerMessage::Ready(_) | RunnerMessage::Paused(_)
                );

                let (response, close_reason) =
                    handle_runner_message(log, context, execution, runner_msg, &mut billing_state)
                        .await?;

                if !is_idle {
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
                bill_final_minutes(log, context, execution.job_id, &mut billing_state).await;
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
    use bencher_schema::model::{
        project::{ProjectId, report::ReportId},
        runner::SourceIp,
        spec::SpecId,
    };

    use super::*;

    fn test_job() -> QueryJob {
        QueryJob {
            id: JobId::try_from_raw(1).unwrap(),
            uuid: JobUuid::default(),
            organization_id: OrganizationId::try_from_raw(1).unwrap(),
            project_id: ProjectId::try_from_raw(1).unwrap(),
            report_id: ReportId::try_from_raw(1).unwrap(),
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
        assert_eq!(BillingDelta::new(&job, 0), None);
    }

    #[test]
    fn billing_delta_first_minute() {
        // 0 seconds of executions → 1 minute (ceiling), no last_billed → delta = 1
        let job = test_job();
        assert_eq!(
            BillingDelta::new(&job, 0),
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
        assert_eq!(BillingDelta::new(&job, 0), None);
    }

    #[test]
    fn billing_delta_exactly_one_minute() {
        // 60 seconds → minute 1, last_billed = None (0) → delta = 1
        let job = test_job();
        assert_eq!(
            BillingDelta::new(&job, 60),
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
        assert_eq!(
            BillingDelta::new(&job, 61),
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
        assert_eq!(
            BillingDelta::new(&job, 180),
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
        assert_eq!(BillingDelta::new(&job, 120), None);
    }

    #[test]
    fn billing_delta_last_billed_exceeds_minutes() {
        // 120 seconds → minute 2, last_billed = 5 → None
        // (can happen if clock skew or final billing already ran)
        let mut job = test_job();
        job.last_billed_minute = Some(5);
        assert_eq!(BillingDelta::new(&job, 120), None);
    }

    #[test]
    fn billing_delta_preserves_organization_id() {
        let mut job = test_job();
        job.organization_id = OrganizationId::try_from_raw(42).unwrap();
        assert_eq!(
            BillingDelta::new(&job, 0),
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
        assert_eq!(
            BillingDelta::new(&job, 3600),
            Some(BillingDelta {
                delta: 2,
                minutes: 60,
                organization_id: job.organization_id
            })
        );
    }

    // --- handle_ready update channel tests ---

    mod ready {
        use std::sync::{
            Arc, Mutex,
            atomic::{AtomicUsize, Ordering},
        };

        use bencher_json::{
            Sha256, UpdateChannel,
            runner::{
                JsonPaused, JsonReady, JsonRunnerMetadata, MAX_PAUSE_REASONS,
                MAX_REPORT_STRING_LEN, MdSyncAction, PauseReason,
            },
        };
        use bencher_schema::context::RunnerUpdate;
        use futures::{SinkExt as _, channel::mpsc};

        use super::super::{Message, ReadyOutcome, ServerMessage, handle_paused, handle_ready};
        use crate::channel::ChannelError;

        fn test_log() -> slog::Logger {
            slog::Logger::root(slog::Discard, slog::o!())
        }

        fn test_clock() -> bencher_json::Clock {
            bencher_json::Clock::Custom(Arc::new(|| bencher_json::DateTime::TEST))
        }

        fn running_checksum() -> Sha256 {
            "a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3"
                .parse()
                .unwrap()
        }

        fn published_checksum() -> Sha256 {
            "b3a8e0e1f9ab1bfe3a36f231f676f78bb30a519d2b21e6c530c0eee8ebb4a5d0"
                .parse()
                .unwrap()
        }

        fn metadata(
            channel: Option<UpdateChannel>,
            version: &str,
            checksum: Option<Sha256>,
        ) -> JsonRunnerMetadata {
            JsonRunnerMetadata {
                os: bencher_json::OperatingSystem::Linux,
                arch: bencher_json::Architecture::X86_64,
                version: version.to_owned(),
                channel,
                checksum,
            }
        }

        /// A checksum fetcher that records requested URLs, counts calls,
        /// and returns a fixed result.
        struct FakeFetcher {
            calls: Arc<AtomicUsize>,
            urls: Arc<Mutex<Vec<url::Url>>>,
            result: Result<Sha256, ()>,
        }

        impl FakeFetcher {
            fn ok(checksum: Sha256) -> Self {
                Self {
                    calls: Arc::new(AtomicUsize::new(0)),
                    urls: Arc::new(Mutex::new(Vec::new())),
                    result: Ok(checksum),
                }
            }

            fn err() -> Self {
                Self {
                    calls: Arc::new(AtomicUsize::new(0)),
                    urls: Arc::new(Mutex::new(Vec::new())),
                    result: Err(()),
                }
            }

            fn call_count(&self) -> usize {
                self.calls.load(Ordering::SeqCst)
            }

            fn requested_urls(&self) -> Vec<url::Url> {
                self.urls.lock().unwrap().clone()
            }

            fn fetch(
                &self,
            ) -> impl Fn(url::Url) -> futures::future::Ready<Result<Sha256, ChannelError>> + use<>
            {
                let calls = self.calls.clone();
                let urls = self.urls.clone();
                let result = self.result.clone();
                move |url| {
                    calls.fetch_add(1, Ordering::SeqCst);
                    urls.lock().unwrap().push(url);
                    futures::future::ready(result.clone().map_err(|()| {
                        ChannelError::UpdateHttpStatus(reqwest::StatusCode::NOT_FOUND)
                    }))
                }
            }
        }

        /// A sink that collects sent WebSocket messages.
        fn test_sink() -> (
            impl futures::Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
            mpsc::Receiver<Message>,
        ) {
            let (tx, rx) = mpsc::channel(8);
            (
                tx.sink_map_err(|_| tokio_tungstenite::tungstenite::Error::ConnectionClosed),
                rx,
            )
        }

        fn sent_update(rx: &mut mpsc::Receiver<Message>) -> ServerMessage {
            let Ok(Message::Text(text)) = rx.try_recv() else {
                panic!("Expected a text message to have been sent");
            };
            serde_json::from_str(&text).unwrap()
        }

        #[tokio::test]
        async fn no_metadata_is_ready() {
            let (mut tx, mut rx) = test_sink();
            let fetcher = FakeFetcher::ok(published_checksum());
            let outcome = handle_ready(
                &test_log(),
                &RunnerUpdate::new(None),
                &test_clock(),
                &mut tx,
                None,
                None,
                fetcher.fetch(),
            )
            .await
            .unwrap();
            assert!(matches!(outcome, ReadyOutcome::Ready(_)));
            assert_eq!(fetcher.call_count(), 0);
            assert!(rx.try_recv().is_err(), "no message should be sent");
        }

        #[tokio::test]
        async fn non_linux_is_ready() {
            let (mut tx, _rx) = test_sink();
            let fetcher = FakeFetcher::ok(published_checksum());
            let mut runner = metadata(Some(UpdateChannel::Stable), "0.0.0", None);
            runner.os = bencher_json::OperatingSystem::Macos;
            let outcome = handle_ready(
                &test_log(),
                &RunnerUpdate::new(None),
                &test_clock(),
                &mut tx,
                None,
                Some(&runner),
                fetcher.fetch(),
            )
            .await
            .unwrap();
            assert!(matches!(outcome, ReadyOutcome::Ready(_)));
            assert_eq!(fetcher.call_count(), 0);
        }

        #[tokio::test]
        async fn stable_version_match_is_ready() {
            let (mut tx, _rx) = test_sink();
            let fetcher = FakeFetcher::ok(published_checksum());
            let runner = metadata(None, bencher_json::BENCHER_API_VERSION, None);
            let outcome = handle_ready(
                &test_log(),
                &RunnerUpdate::new(None),
                &test_clock(),
                &mut tx,
                None,
                Some(&runner),
                fetcher.fetch(),
            )
            .await
            .unwrap();
            assert!(matches!(outcome, ReadyOutcome::Ready(_)));
            assert_eq!(fetcher.call_count(), 0);
        }

        #[tokio::test]
        async fn absent_channel_defaults_to_stable() {
            // A legacy runner sends no channel; a version mismatch must take
            // the stable update path.
            let (mut tx, mut rx) = test_sink();
            let fetcher = FakeFetcher::ok(published_checksum());
            let runner = metadata(None, "0.0.0", None);
            let outcome = handle_ready(
                &test_log(),
                &RunnerUpdate::new(None),
                &test_clock(),
                &mut tx,
                None,
                Some(&runner),
                fetcher.fetch(),
            )
            .await
            .unwrap();
            assert!(matches!(outcome, ReadyOutcome::UpdateSent));

            let ServerMessage::Update { version, .. } = sent_update(&mut rx) else {
                panic!("Expected Update message");
            };
            assert_eq!(version, bencher_json::BENCHER_API_VERSION);
        }

        #[tokio::test]
        async fn stable_version_mismatch_sends_update() {
            let (mut tx, mut rx) = test_sink();
            let fetcher = FakeFetcher::ok(published_checksum());
            let runner = metadata(Some(UpdateChannel::Stable), "0.0.0", None);
            let outcome = handle_ready(
                &test_log(),
                &RunnerUpdate::new(None),
                &test_clock(),
                &mut tx,
                None,
                Some(&runner),
                fetcher.fetch(),
            )
            .await
            .unwrap();
            assert!(matches!(outcome, ReadyOutcome::UpdateSent));

            let server_version = bencher_json::BENCHER_API_VERSION;
            let expected_url = format!(
                "https://github.com/bencherdev/bencher/releases/download/v{server_version}/runner-v{server_version}-linux-x86-64"
            );
            assert_eq!(
                fetcher.requested_urls(),
                vec![expected_url.parse().unwrap()]
            );

            let ServerMessage::Update {
                version,
                url,
                checksum,
            } = sent_update(&mut rx)
            else {
                panic!("Expected Update message");
            };
            assert_eq!(version, server_version);
            assert_eq!(url.as_str(), expected_url);
            assert_eq!(checksum, published_checksum());
        }

        #[tokio::test]
        async fn stable_checksum_fetch_failure_is_ready() {
            let (mut tx, mut rx) = test_sink();
            let fetcher = FakeFetcher::err();
            let runner = metadata(Some(UpdateChannel::Stable), "0.0.0", None);
            let outcome = handle_ready(
                &test_log(),
                &RunnerUpdate::new(None),
                &test_clock(),
                &mut tx,
                None,
                Some(&runner),
                fetcher.fetch(),
            )
            .await
            .unwrap();
            assert!(matches!(outcome, ReadyOutcome::Ready(_)));
            assert_eq!(fetcher.call_count(), 1);
            assert!(rx.try_recv().is_err(), "no message should be sent");
        }

        #[tokio::test]
        async fn canary_checksum_match_is_ready() {
            let (mut tx, _rx) = test_sink();
            let fetcher = FakeFetcher::ok(running_checksum());
            let runner = metadata(
                Some(UpdateChannel::Canary),
                "0.0.0",
                Some(running_checksum()),
            );
            let outcome = handle_ready(
                &test_log(),
                &RunnerUpdate::new(None),
                &test_clock(),
                &mut tx,
                None,
                Some(&runner),
                fetcher.fetch(),
            )
            .await
            .unwrap();
            assert!(matches!(outcome, ReadyOutcome::Ready(_)));
            assert_eq!(fetcher.call_count(), 1);
        }

        #[tokio::test]
        async fn canary_checksum_mismatch_sends_update() {
            let (mut tx, mut rx) = test_sink();
            let fetcher = FakeFetcher::ok(published_checksum());
            let runner = metadata(
                Some(UpdateChannel::Canary),
                "0.0.0",
                Some(running_checksum()),
            );
            let outcome = handle_ready(
                &test_log(),
                &RunnerUpdate::new(None),
                &test_clock(),
                &mut tx,
                None,
                Some(&runner),
                fetcher.fetch(),
            )
            .await
            .unwrap();
            assert!(matches!(outcome, ReadyOutcome::UpdateSent));

            let expected_url = "https://download.bencher.dev/canary/runner-canary-linux-x86-64";
            let ServerMessage::Update {
                version,
                url,
                checksum,
            } = sent_update(&mut rx)
            else {
                panic!("Expected Update message");
            };
            assert_eq!(version, "canary");
            assert_eq!(url.as_str(), expected_url);
            assert_eq!(checksum, published_checksum());
        }

        #[tokio::test]
        async fn canary_no_runner_checksum_is_ready() {
            let (mut tx, mut rx) = test_sink();
            let fetcher = FakeFetcher::ok(published_checksum());
            let runner = metadata(Some(UpdateChannel::Canary), "0.0.0", None);
            let outcome = handle_ready(
                &test_log(),
                &RunnerUpdate::new(None),
                &test_clock(),
                &mut tx,
                None,
                Some(&runner),
                fetcher.fetch(),
            )
            .await
            .unwrap();
            assert!(matches!(outcome, ReadyOutcome::Ready(_)));
            assert_eq!(fetcher.call_count(), 0);
            assert!(rx.try_recv().is_err(), "no message should be sent");
        }

        #[tokio::test]
        async fn canary_checksum_fetch_failure_is_ready() {
            let (mut tx, mut rx) = test_sink();
            let fetcher = FakeFetcher::err();
            let runner = metadata(
                Some(UpdateChannel::Canary),
                "0.0.0",
                Some(running_checksum()),
            );
            let outcome = handle_ready(
                &test_log(),
                &RunnerUpdate::new(None),
                &test_clock(),
                &mut tx,
                None,
                Some(&runner),
                fetcher.fetch(),
            )
            .await
            .unwrap();
            assert!(matches!(outcome, ReadyOutcome::Ready(_)));
            assert_eq!(fetcher.call_count(), 1);
            assert!(rx.try_recv().is_err(), "no message should be sent");
        }

        #[tokio::test]
        async fn canary_checksum_is_cached_within_ttl() {
            let (mut tx, _rx) = test_sink();
            let fetcher = FakeFetcher::ok(running_checksum());
            let runner_update = RunnerUpdate::new(None);
            let clock = test_clock();
            let runner = metadata(
                Some(UpdateChannel::Canary),
                "0.0.0",
                Some(running_checksum()),
            );

            for _ in 0..3 {
                let outcome = handle_ready(
                    &test_log(),
                    &runner_update,
                    &clock,
                    &mut tx,
                    None,
                    Some(&runner),
                    fetcher.fetch(),
                )
                .await
                .unwrap();
                assert!(matches!(outcome, ReadyOutcome::Ready(_)));
            }

            assert_eq!(
                fetcher.call_count(),
                1,
                "checksum should be fetched once and then served from cache"
            );
        }

        fn paused(runner: JsonRunnerMetadata, reasons: usize) -> JsonPaused {
            JsonPaused {
                reasons: vec![PauseReason::Other; reasons],
                since: bencher_json::DateTime::TEST,
                ready: JsonReady::new(None, Some(runner)),
            }
        }

        #[tokio::test]
        async fn paused_stable_version_mismatch_sends_update() {
            let (mut tx, mut rx) = test_sink();
            let fetcher = FakeFetcher::ok(published_checksum());
            let outcome = handle_paused(
                &test_log(),
                &RunnerUpdate::new(None),
                &test_clock(),
                &mut tx,
                paused(metadata(None, "0.0.0", None), 1),
                fetcher.fetch(),
            )
            .await
            .unwrap();
            assert!(matches!(outcome, ReadyOutcome::UpdateSent));
            let ServerMessage::Update {
                version, checksum, ..
            } = sent_update(&mut rx)
            else {
                panic!("Expected an Update message");
            };
            assert_eq!(version, bencher_json::BENCHER_API_VERSION);
            assert_eq!(checksum, published_checksum());
        }

        #[tokio::test]
        async fn paused_reasons_are_capped() {
            let (mut tx, _rx) = test_sink();
            let fetcher = FakeFetcher::ok(published_checksum());
            let outcome = handle_paused(
                &test_log(),
                &RunnerUpdate::new(None),
                &test_clock(),
                &mut tx,
                paused(
                    metadata(None, bencher_json::BENCHER_API_VERSION, None),
                    MAX_PAUSE_REASONS + 1,
                ),
                fetcher.fetch(),
            )
            .await
            .unwrap();
            let ReadyOutcome::Paused(pause) = outcome else {
                panic!("Expected Paused");
            };
            assert_eq!(pause.reasons.len(), MAX_PAUSE_REASONS);
        }

        // The pause start is logged from this outcome, so a runner's long strings are cut before it.
        #[tokio::test]
        async fn paused_reason_strings_are_capped() {
            let (mut tx, _rx) = test_sink();
            let fetcher = FakeFetcher::ok(published_checksum());
            let long = "m".repeat(1000);
            let capped = "m".repeat(MAX_REPORT_STRING_LEN);
            let mut pause = paused(metadata(None, bencher_json::BENCHER_API_VERSION, None), 0);
            pause.reasons = vec![PauseReason::Raid {
                array: long.clone(),
                action: MdSyncAction::Other(long.clone()),
                done: None,
                total: None,
                speed: None,
            }];
            let outcome = handle_paused(
                &test_log(),
                &RunnerUpdate::new(None),
                &test_clock(),
                &mut tx,
                pause,
                fetcher.fetch(),
            )
            .await
            .unwrap();
            let ReadyOutcome::Paused(pause) = outcome else {
                panic!("Expected Paused");
            };
            let [
                PauseReason::Raid {
                    array,
                    action: MdSyncAction::Other(action),
                    ..
                },
            ] = pause.reasons.as_slice()
            else {
                panic!("Expected one raid reason, got {:?}", pause.reasons);
            };
            assert_eq!(array, &capped);
            assert_eq!(action, &capped);
        }
    }
}
