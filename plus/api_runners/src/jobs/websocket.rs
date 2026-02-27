//! WebSocket channel for runner job execution.
//!
//! Provides a persistent connection for heartbeat and status updates during job execution.

use bencher_json::{JobStatus, JobUuid, RunnerResourceId, runner::JsonIterationOutput};
use bencher_oci_storage::OciStorageError;
use bencher_schema::{
    auth_conn,
    context::ApiContext,
    error::{conflict_error, forbidden_error, resource_not_found_err},
    model::runner::{JobId, QueryJob, UpdateJob, job::spawn_heartbeat_timeout},
    schema, write_conn,
};
use diesel::{BoolExpressionMethods as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::WebsocketConnectionRaw;
use dropshot::{Path, RequestContext, WebsocketChannelResult, WebsocketConnection, channel};
use futures::{SinkExt as _, StreamExt as _};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use tokio_tungstenite::tungstenite::{
    Message,
    protocol::{CloseFrame, Role, WebSocketConfig, frame::coding::CloseCode},
};

use crate::runner_token::RunnerToken;

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

/// Path parameters for the job WebSocket endpoint.
#[derive(Deserialize, JsonSchema)]
pub struct RunnerJobParams {
    /// The slug or UUID for a runner.
    pub runner: RunnerResourceId,
    /// The UUID for a job.
    pub job: JobUuid,
}

/// Messages sent from the runner to the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum RunnerMessage {
    /// Job setup complete, benchmark execution starting.
    Running,
    /// Periodic heartbeat, keeps job alive and triggers billing.
    Heartbeat,
    /// Benchmark completed successfully.
    Completed {
        /// Per-iteration results
        results: Vec<JsonIterationOutput>,
    },
    /// Benchmark failed.
    Failed {
        /// Per-iteration results collected before failure
        results: Vec<JsonIterationOutput>,
        /// Error description
        error: String,
    },
    /// Acknowledge cancellation from server.
    Canceled,
}

/// Messages sent from the server to the runner.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ServerMessage {
    /// Acknowledge received message.
    Ack,
    /// Job was canceled, stop execution immediately.
    Cancel,
}

/// WebSocket channel for job execution
///
/// ➕ Bencher Plus: Establishes a persistent connection for heartbeat and status updates.
/// Authentication is via runner token in the Authorization header.
#[channel {
    protocol = WEBSOCKETS,
    path = "/v0/runners/{runner}/jobs/{job}",
    tags = ["runners"]
}]
pub async fn runner_job_channel(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<RunnerJobParams>,
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

    // Verify job exists and is claimed by this runner
    let job = QueryJob::from_uuid(auth_conn!(context), path_params.job)?;

    if job.runner_id != Some(runner_token.runner_id) {
        return Err(forbidden_error("Job is not assigned to this runner").into());
    }

    // Only allow channel for claimed or running jobs (running allows reconnection)
    if !matches!(job.status, JobStatus::Claimed | JobStatus::Running) {
        return Err(conflict_error(format!(
            "Cannot open channel for job in {:?} status, expected Claimed or Running",
            job.status
        ))
        .into());
    }

    // Upgrade to WebSocket and handle messages
    let mut ws_config = WebSocketConfig::default();
    ws_config.max_message_size = Some(context.request_body_max_bytes);
    ws_config.max_frame_size = Some(context.request_body_max_bytes);
    let ws_stream = tokio_tungstenite::WebSocketStream::from_raw_socket(
        conn.into_inner(),
        Role::Server,
        Some(ws_config),
    )
    .await;

    let heartbeat_timeout = context.heartbeat_timeout;

    handle_websocket(&log, context, &job, ws_stream, heartbeat_timeout).await?;

    // After WS disconnect, check if job is still in-flight and spawn a timeout task
    let job = QueryJob::get(auth_conn!(context), job.id)?;
    if !job.status.is_terminal() {
        slog::info!(log, "WS disconnected for in-flight job, spawning heartbeat timeout"; "job_id" => ?job.id);
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

    Ok(())
}

/// Handle WebSocket messages for a job.
///
/// The heartbeat timeout only resets on valid protocol messages from the runner
/// (Running, Heartbeat, Completed, Failed, Canceled). Ping/Pong frames, invalid
/// JSON, and other non-protocol messages do NOT reset the timeout.
async fn handle_websocket(
    log: &slog::Logger,
    context: &ApiContext,
    job: &QueryJob,
    ws_stream: tokio_tungstenite::WebSocketStream<WebsocketConnectionRaw>,
    heartbeat_timeout: Duration,
) -> Result<(), ChannelError> {
    let (mut tx, mut rx) = ws_stream.split();
    let mut last_heartbeat = tokio::time::Instant::now();

    loop {
        let remaining = heartbeat_timeout
            .checked_sub(last_heartbeat.elapsed())
            .unwrap_or(Duration::ZERO);

        let msg_result = match tokio::time::timeout(remaining, rx.next()).await {
            Ok(Some(msg_result)) => msg_result,
            Ok(None) => {
                // Stream ended (client disconnected cleanly)
                break;
            },
            Err(_elapsed) => {
                let reason = handle_timeout(log, context, job.id).await?;
                if let Err(e) = tx
                    .send(Message::Close(Some(CloseFrame {
                        code: CloseCode::Policy,
                        reason: reason.into(),
                    })))
                    .await
                {
                    slog::debug!(log, "Failed to send close frame"; "error" => %e);
                }
                break;
            },
        };

        let msg = match msg_result {
            Ok(msg) => msg,
            Err(e) => {
                slog::warn!(log, "WebSocket error"; "error" => %e);
                break;
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

                let response = handle_runner_message(log, context, job, &runner_msg).await?;

                // Reset heartbeat only on valid protocol messages
                last_heartbeat = tokio::time::Instant::now();

                let response_text = serde_json::to_string(&response)?;
                tx.send(Message::Text(response_text.into())).await?;

                // Close the connection on terminal messages
                let close_reason = terminal_close_reason(&response, &runner_msg);
                if let Some(reason) = close_reason {
                    if let Err(e) = tx
                        .send(Message::Close(Some(CloseFrame {
                            code: CloseCode::Normal,
                            reason: reason.into(),
                        })))
                        .await
                    {
                        slog::debug!(log, "Failed to send close frame"; "error" => %e);
                    }
                    break;
                }
            },
            Message::Close(_) => {
                slog::info!(log, "WebSocket closed by client");
                break;
            },
            Message::Ping(data) => {
                // Respond to Ping but do NOT reset heartbeat timeout
                tx.send(Message::Pong(data)).await?;
            },
            Message::Binary(_) | Message::Pong(_) | Message::Frame(_) => {
                // Ignore binary messages, pong responses, and raw frames
                // Do NOT reset heartbeat timeout
            },
        }
    }

    Ok(())
}

/// Handle a heartbeat timeout by reading the job and deciding the right status.
///
/// If the job has exceeded its configured timeout + grace period, it is marked `Canceled`
/// (ran too long). Otherwise it is marked `Failed` (lost contact with runner).
/// Returns the close reason string for the WebSocket close frame.
async fn handle_timeout(
    log: &slog::Logger,
    context: &ApiContext,
    job_id: JobId,
) -> Result<&'static str, ChannelError> {
    let job: QueryJob = schema::job::table
        .filter(schema::job::id.eq(job_id))
        .first(auth_conn!(context))?;

    if job.status.is_terminal() {
        slog::info!(log, "Heartbeat timeout: job already in terminal state"; "job_id" => ?job_id);
        return Ok("heartbeat timeout");
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
            (JobStatus::Canceled, "job timeout exceeded")
        } else {
            (JobStatus::Failed, "heartbeat timeout")
        }
    } else {
        (JobStatus::Failed, "heartbeat timeout")
    };

    slog::warn!(log, "Marking job"; "job_id" => ?job_id, "status" => ?status, "reason" => reason);
    let update = UpdateJob {
        status: Some(status),
        completed: Some(Some(now)),
        modified: Some(now),
        ..Default::default()
    };
    let updated = diesel::update(
        schema::job::table
            .filter(schema::job::id.eq(job_id))
            .filter(
                schema::job::status
                    .eq(JobStatus::Claimed)
                    .or(schema::job::status.eq(JobStatus::Running)),
            ),
    )
    .set(&update)
    .execute(write_conn!(context))?;
    if updated == 0 {
        slog::info!(log, "Timeout: job already in terminal state"; "job_id" => ?job_id);
    }
    Ok(reason)
}

/// Check if a response/message pair represents a terminal state that should close the connection.
fn terminal_close_reason(
    response: &ServerMessage,
    runner_msg: &RunnerMessage,
) -> Option<&'static str> {
    if matches!(response, ServerMessage::Cancel) {
        return Some("job canceled");
    }
    if matches!(runner_msg, RunnerMessage::Completed { .. }) {
        return Some("job completed");
    }
    if matches!(runner_msg, RunnerMessage::Failed { .. }) {
        return Some("job failed");
    }
    if matches!(runner_msg, RunnerMessage::Canceled) {
        return Some("job canceled by runner");
    }
    None
}

/// Handle a message from the runner and return the appropriate response.
async fn handle_runner_message(
    log: &slog::Logger,
    context: &ApiContext,
    job: &QueryJob,
    msg: &RunnerMessage,
) -> Result<ServerMessage, ChannelError> {
    match msg {
        RunnerMessage::Running => {
            slog::info!(log, "Job running"; "job_id" => ?job.id);
            if let Some(cancel) = handle_running(log, context, job.id).await? {
                return Ok(cancel);
            }
        },
        RunnerMessage::Heartbeat => {
            slog::debug!(log, "Job heartbeat"; "job_id" => ?job.id);
            if let Some(cancel) = handle_heartbeat(log, context, job.id).await? {
                return Ok(cancel);
            }
        },
        RunnerMessage::Completed { results } => {
            slog::info!(log, "Job completed"; "job_id" => ?job.id, "iterations" => results.len());
            handle_completed(log, context, job, results.clone()).await?;
        },
        RunnerMessage::Failed { results, error } => {
            slog::warn!(log, "Job failed"; "job_id" => ?job.id, "error" => &error);
            handle_failed(log, context, job, results.clone(), error.clone()).await?;
        },
        RunnerMessage::Canceled => {
            slog::info!(log, "Job cancellation acknowledged"; "job_id" => ?job.id);
            handle_canceled(log, context, job.id).await?;
        },
    }

    Ok(ServerMessage::Ack)
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
    let reconnect_update = UpdateJob {
        last_heartbeat: Some(Some(now)),
        modified: Some(now),
        ..Default::default()
    };
    let updated = diesel::update(
        schema::job::table
            .filter(schema::job::id.eq(job_id))
            .filter(schema::job::status.eq(JobStatus::Running)),
    )
    .set(&reconnect_update)
    .execute(write_conn!(context))?;

    if updated > 0 {
        slog::info!(log, "Runner reconnected to running job"; "job_id" => ?job_id);
        return Ok(None);
    }

    // Try normal transition: Claimed -> Running
    let transition_update = UpdateJob {
        status: Some(JobStatus::Running),
        started: Some(Some(now)),
        last_heartbeat: Some(Some(now)),
        modified: Some(now),
        ..Default::default()
    };
    let updated = diesel::update(
        schema::job::table
            .filter(schema::job::id.eq(job_id))
            .filter(schema::job::status.eq(JobStatus::Claimed)),
    )
    .set(&transition_update)
    .execute(write_conn!(context))?;

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
            let cancel_update = UpdateJob {
                status: Some(JobStatus::Canceled),
                completed: Some(Some(now)),
                modified: Some(now),
                ..Default::default()
            };
            let updated = diesel::update(
                schema::job::table
                    .filter(schema::job::id.eq(job_id))
                    .filter(
                        schema::job::status
                            .eq(JobStatus::Claimed)
                            .or(schema::job::status.eq(JobStatus::Running)),
                    ),
            )
            .set(&cancel_update)
            .execute(write_conn!(context))?;
            if updated > 0 {
                #[cfg(feature = "otel")]
                bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerJobUpdate(
                    bencher_otel::JobStatusKind::Canceled,
                ));
            }
            return Ok(Some(ServerMessage::Cancel));
        }
    }

    let update = UpdateJob {
        last_heartbeat: Some(Some(now)),
        modified: Some(now),
        ..Default::default()
    };

    // It is okay to wait until here to get the write lock
    // Worst case, we add an extra write if the job was canceled between reads
    // Use status filter to avoid overwriting a concurrent cancellation
    diesel::update(
        schema::job::table
            .filter(schema::job::id.eq(job_id))
            .filter(
                schema::job::status
                    .eq(JobStatus::Claimed)
                    .or(schema::job::status.eq(JobStatus::Running)),
            ),
    )
    .set(&update)
    .execute(write_conn!(context))?;

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

    let update = UpdateJob {
        status: Some(JobStatus::Completed),
        completed: Some(Some(now)),
        modified: Some(now),
        ..Default::default()
    };

    let updated = diesel::update(
        schema::job::table
            .filter(schema::job::id.eq(job.id))
            .filter(schema::job::status.eq(JobStatus::Running)),
    )
    .set(&update)
    .execute(write_conn!(context))?;

    if updated == 0 {
        // Re-read the job to determine what happened
        let current_job: QueryJob = schema::job::table
            .filter(schema::job::id.eq(job.id))
            .first(auth_conn!(context))
            .map_err(resource_not_found_err!(Job, job.id))?;

        if current_job.status == JobStatus::Completed {
            slog::debug!(log, "Job already completed (idempotent duplicate)"; "job_id" => ?job.id);
            return Ok(());
        }
        if current_job.status.is_terminal() {
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
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerJobUpdate(
        bencher_otel::JobStatusKind::Completed,
    ));

    // Store output in blob storage (best-effort)
    let job_output = bencher_json::runner::JsonJobOutput {
        results: results.clone(),
        error: None,
    };
    if let Err(e) = store_job_output(context, job, &job_output).await {
        slog::error!(log, "Failed to store job output"; "job_id" => ?job.id, "error" => %e);
    }

    // Process benchmark results into the report
    if let Err(e) = job.process_results(log, context, &results, now).await {
        slog::error!(log, "Failed to process job results"; "job_id" => ?job.id, "error" => %e);
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

    let update = UpdateJob {
        status: Some(JobStatus::Failed),
        completed: Some(Some(now)),
        modified: Some(now),
        ..Default::default()
    };

    let updated = diesel::update(
        schema::job::table
            .filter(schema::job::id.eq(job.id))
            .filter(
                schema::job::status
                    .eq(JobStatus::Claimed)
                    .or(schema::job::status.eq(JobStatus::Running)),
            ),
    )
    .set(&update)
    .execute(write_conn!(context))?;

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
        if current_job.status.is_terminal() {
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
    let update = UpdateJob {
        status: Some(JobStatus::Canceled),
        completed: Some(Some(now)),
        modified: Some(now),
        ..Default::default()
    };

    let updated = diesel::update(
        schema::job::table
            .filter(schema::job::id.eq(job_id))
            .filter(
                schema::job::status
                    .eq(JobStatus::Claimed)
                    .or(schema::job::status.eq(JobStatus::Running)),
            ),
    )
    .set(&update)
    .execute(write_conn!(context))?;

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
