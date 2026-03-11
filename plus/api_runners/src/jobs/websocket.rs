//! WebSocket message handlers for runner job execution.
//!
//! Provides handler functions for job lifecycle messages (Running, Heartbeat,
//! Completed, Failed, Canceled). Used by the channel endpoint.

use bencher_json::{
    JobStatus,
    runner::{CloseReason, JsonIterationOutput, RunnerMessage, ServerMessage},
};
use bencher_oci_storage::OciStorageError;
use bencher_schema::{
    auth_conn,
    context::ApiContext,
    error::resource_not_found_err,
    model::runner::{JobId, QueryJob, UpdateJob},
    schema, write_conn,
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};

/// Errors from WebSocket channel operations during runner job execution.
#[derive(Debug, thiserror::Error)]
pub(crate) enum ChannelError {
    #[error("{0}")]
    Http(#[from] dropshot::HttpError),

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
pub(crate) async fn handle_timeout(
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
        #[expect(
            clippy::cast_possible_wrap,
            reason = "timeout max i32::MAX + grace period fits in i64"
        )]
        let limit = u64::from(u32::from(job.timeout)) as i64
            + context.job_timeout_grace_period.as_secs() as i64;
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
pub(crate) async fn handle_runner_message(
    log: &slog::Logger,
    context: &ApiContext,
    job: &QueryJob,
    msg: RunnerMessage,
) -> Result<(ServerMessage, Option<CloseReason>), ChannelError> {
    match msg {
        RunnerMessage::Ready { .. } => {
            slog::warn!(log, "Unexpected Ready message during job execution"; "job_id" => ?job.id);
            // Ignore Ready during execution — runner should not send it here
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
        #[expect(
            clippy::cast_possible_wrap,
            reason = "timeout max i32::MAX + grace period fits in i64"
        )]
        let limit = u64::from(u32::from(job.timeout)) as i64
            + context.job_timeout_grace_period.as_secs() as i64;
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
