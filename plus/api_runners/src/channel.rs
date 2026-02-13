//! WebSocket channel for runner job execution.
//!
//! Provides a persistent connection for heartbeat and status updates during job execution.

use std::collections::HashMap;

use bencher_json::{DateTime, JobStatus, JobUuid, RunnerResourceId};
use bencher_schema::{
    auth_conn,
    context::ApiContext,
    error::{conflict_error, forbidden_error, resource_not_found_err},
    model::runner::{JobId, QueryJob, UpdateJob, job::spawn_heartbeat_timeout},
    schema, write_conn,
};
use camino::Utf8PathBuf;
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

/// Path parameters for the job channel endpoint.
#[derive(Deserialize, JsonSchema)]
pub struct RunnerJobChannelParams {
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
        exit_code: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        stdout: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        stderr: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        output: Option<HashMap<Utf8PathBuf, String>>,
    },
    /// Benchmark failed.
    Failed {
        #[serde(skip_serializing_if = "Option::is_none")]
        exit_code: Option<i32>,
        error: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        stdout: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        stderr: Option<String>,
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
/// Establishes a persistent connection for heartbeat and status updates.
/// Authentication is via runner token in the Authorization header.
#[channel {
    protocol = WEBSOCKETS,
    path = "/v0/runners/{runner}/jobs/{job}/channel",
    tags = ["runners"]
}]
pub async fn runner_job_channel(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<RunnerJobChannelParams>,
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

    let job_id = job.id;

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

    handle_websocket(&log, context, job_id, ws_stream, heartbeat_timeout).await?;

    // After WS disconnect, check if job is still in-flight and spawn a timeout task
    let job = QueryJob::get(auth_conn!(context), job_id)?;
    if !job.status.is_terminal() {
        slog::info!(log, "WS disconnected for in-flight job, spawning heartbeat timeout"; "job_id" => ?job_id);
        spawn_heartbeat_timeout(
            log,
            heartbeat_timeout,
            context.database.connection.clone(),
            job_id,
            &context.heartbeat_tasks,
            context.job_timeout_grace_period,
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
    job_id: JobId,
    ws_stream: tokio_tungstenite::WebSocketStream<WebsocketConnectionRaw>,
    heartbeat_timeout: Duration,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
                let reason = handle_timeout(log, context, job_id).await?;
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

                let response = handle_runner_message(log, context, job_id, &runner_msg).await?;

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
) -> Result<&'static str, Box<dyn std::error::Error + Send + Sync>> {
    let job: QueryJob = schema::job::table
        .filter(schema::job::id.eq(job_id))
        .first(auth_conn!(context))?;

    if job.status.is_terminal() {
        slog::info!(log, "Heartbeat timeout: job already in terminal state"; "job_id" => ?job_id);
        return Ok("heartbeat timeout");
    }

    let now = DateTime::now();

    let (status, reason) = if let Some(started) = job.started {
        let elapsed = (now.timestamp() - started.timestamp()).max(0);
        #[expect(
            clippy::cast_possible_wrap,
            reason = "timeout max 86400 + grace period fits in i64"
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
    job_id: JobId,
    msg: &RunnerMessage,
) -> Result<ServerMessage, Box<dyn std::error::Error + Send + Sync>> {
    match msg {
        RunnerMessage::Running => {
            slog::info!(log, "Job running"; "job_id" => ?job_id);
            handle_running(log, context, job_id).await?;
        },
        RunnerMessage::Heartbeat => {
            slog::debug!(log, "Job heartbeat"; "job_id" => ?job_id);
            if let Some(cancel) = handle_heartbeat(context, job_id).await? {
                return Ok(cancel);
            }
        },
        RunnerMessage::Completed {
            exit_code,
            stdout,
            stderr,
            output,
        } => {
            slog::info!(log, "Job completed"; "job_id" => ?job_id, "exit_code" => exit_code);
            handle_completed(
                log,
                context,
                job_id,
                *exit_code,
                stdout.clone(),
                stderr.clone(),
                output.clone(),
            )
            .await?;
        },
        RunnerMessage::Failed {
            exit_code,
            error,
            stdout,
            stderr,
        } => {
            slog::warn!(log, "Job failed"; "job_id" => ?job_id, "exit_code" => ?exit_code, "error" => &error);
            handle_failed(
                log,
                context,
                job_id,
                *exit_code,
                stdout.clone(),
                stderr.clone(),
            )
            .await?;
        },
        RunnerMessage::Canceled => {
            slog::info!(log, "Job cancellation acknowledged"; "job_id" => ?job_id);
            handle_canceled(log, context, job_id).await?;
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
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let now = DateTime::now();

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
        return Ok(());
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
        return Ok(());
    }

    // Neither matched — check if job was canceled
    let current_job: QueryJob = schema::job::table
        .filter(schema::job::id.eq(job_id))
        .first(auth_conn!(context))?;
    if current_job.status == JobStatus::Canceled {
        slog::info!(log, "Job was canceled before Running transition"; "job_id" => ?job_id);
        return Ok(());
    }
    slog::warn!(log, "Unexpected job state during Running transition"; "job_id" => ?job_id, "status" => ?current_job.status);
    Ok(())
}

/// Handle a Heartbeat message: update `last_heartbeat` and check for cancellation.
async fn handle_heartbeat(
    context: &ApiContext,
    job_id: JobId,
) -> Result<Option<ServerMessage>, Box<dyn std::error::Error + Send + Sync>> {
    let now = DateTime::now();

    let job: QueryJob = schema::job::table
        .filter(schema::job::id.eq(job_id))
        .first(auth_conn!(context))
        .map_err(resource_not_found_err!(Job, job_id))?;

    // Check if job was canceled
    if job.status == JobStatus::Canceled {
        return Ok(Some(ServerMessage::Cancel));
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

/// Handle a Completed message: transition job from Running to Completed.
///
/// Uses a status filter on the UPDATE to avoid TOCTOU races.
async fn handle_completed(
    log: &slog::Logger,
    context: &ApiContext,
    job_id: JobId,
    exit_code: i32,
    stdout: Option<String>,
    stderr: Option<String>,
    output: Option<HashMap<Utf8PathBuf, String>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let now = DateTime::now();

    let update = UpdateJob {
        status: Some(JobStatus::Completed),
        completed: Some(Some(now)),
        exit_code: Some(Some(exit_code)),
        modified: Some(now),
        ..Default::default()
    };

    let updated = diesel::update(
        schema::job::table
            .filter(schema::job::id.eq(job_id))
            .filter(schema::job::status.eq(JobStatus::Running)),
    )
    .set(&update)
    .execute(write_conn!(context))?;

    if updated == 0 {
        slog::warn!(log, "Invalid state transition to Completed (concurrent state change)"; "job_id" => ?job_id);
        return Err(format!(
            "Invalid state transition to Completed for job {job_id:?}, expected Running"
        )
        .into());
    }

    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerJobUpdate(
        bencher_otel::JobStatusKind::Completed,
    ));

    // TODO: Store output somewhere (job table or separate results table)
    drop(stdout);
    drop(stderr);
    drop(output);

    Ok(())
}

/// Handle a Failed message: transition job from Claimed or Running to Failed.
///
/// Uses a status filter on the UPDATE to avoid TOCTOU races.
async fn handle_failed(
    log: &slog::Logger,
    context: &ApiContext,
    job_id: JobId,
    exit_code: Option<i32>,
    stdout: Option<String>,
    stderr: Option<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let now = DateTime::now();

    let update = UpdateJob {
        status: Some(JobStatus::Failed),
        completed: Some(Some(now)),
        exit_code: Some(exit_code),
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
        slog::warn!(log, "Invalid state transition to Failed (concurrent state change)"; "job_id" => ?job_id);
        return Err(format!(
            "Invalid state transition to Failed for job {job_id:?}, expected Claimed or Running"
        )
        .into());
    }

    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerJobUpdate(
        bencher_otel::JobStatusKind::Failed,
    ));

    // TODO: Store output somewhere (job table or separate results table)
    drop(stdout);
    drop(stderr);

    Ok(())
}

/// Handle a Canceled message: runner acknowledges cancellation, ensure job is in Canceled state.
///
/// Uses a status filter on the UPDATE to avoid TOCTOU races. If zero rows updated,
/// re-reads to check whether the job is already Canceled (idempotent success).
async fn handle_canceled(
    log: &slog::Logger,
    context: &ApiContext,
    job_id: JobId,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let now = DateTime::now();

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
    Err(format!(
        "Invalid state transition from {:?} to Canceled for job {job_id:?}, expected Claimed or Running",
        job.status
    )
    .into())
}
