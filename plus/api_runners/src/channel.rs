//! WebSocket channel for runner job execution.
//!
//! Provides a persistent connection for heartbeat and status updates during job execution.

use bencher_json::{DateTime, JobStatus, JobUuid, RunnerResourceId};
use bencher_schema::{
    auth_conn,
    context::ApiContext,
    error::{forbidden_error, resource_not_found_err},
    model::runner::{JobId, QueryJob, UpdateJob, job::spawn_heartbeat_timeout},
    schema, write_conn,
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::WebsocketConnectionRaw;
use dropshot::{Path, RequestContext, WebsocketChannelResult, WebsocketConnection, channel};
use futures::{SinkExt as _, StreamExt as _};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use tokio_tungstenite::tungstenite::{
    Message,
    protocol::{Role, WebSocketConfig},
};

use crate::runner_token::RunnerToken;

/// Default heartbeat timeout when the `plus` feature is not enabled.
#[cfg(not(feature = "plus"))]
const DEFAULT_HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(90);

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
        output: Option<String>,
    },
    /// Benchmark failed.
    Failed {
        #[serde(skip_serializing_if = "Option::is_none")]
        exit_code: Option<i32>,
        error: String,
    },
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
        return Err(forbidden_error(format!(
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

    #[cfg(feature = "plus")]
    let heartbeat_timeout = context.heartbeat_timeout;
    #[cfg(not(feature = "plus"))]
    let heartbeat_timeout = DEFAULT_HEARTBEAT_TIMEOUT;

    handle_websocket(&log, context, job_id, ws_stream, heartbeat_timeout).await?;

    // After WS disconnect, check if job is still in-flight and spawn a timeout task
    let job = QueryJob::get(write_conn!(context), job_id)?;
    if !job.status.is_terminal() {
        slog::info!(log, "WS disconnected for in-flight job, spawning heartbeat timeout"; "job_id" => ?job_id);
        spawn_heartbeat_timeout(
            log,
            heartbeat_timeout,
            context.database.connection.clone(),
            job_id,
        );
    }

    Ok(())
}

/// Handle WebSocket messages for a job.
async fn handle_websocket(
    log: &slog::Logger,
    context: &ApiContext,
    job_id: JobId,
    ws_stream: tokio_tungstenite::WebSocketStream<WebsocketConnectionRaw>,
    heartbeat_timeout: Duration,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (mut tx, mut rx) = ws_stream.split();

    loop {
        let msg_result = match tokio::time::timeout(heartbeat_timeout, rx.next()).await {
            Ok(Some(msg_result)) => msg_result,
            Ok(None) => {
                // Stream ended (client disconnected cleanly)
                break;
            },
            Err(_elapsed) => {
                slog::warn!(log, "Heartbeat timeout, marking job as failed"; "job_id" => ?job_id);
                let now = DateTime::now();
                let update = UpdateJob {
                    status: Some(JobStatus::Failed),
                    completed: Some(Some(now)),
                    modified: Some(now),
                    ..Default::default()
                };
                diesel::update(schema::job::table.filter(schema::job::id.eq(job_id)))
                    .set(&update)
                    .execute(write_conn!(context))?;
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
                        continue;
                    },
                };

                let response = handle_runner_message(log, context, job_id, runner_msg).await?;

                let response_text = serde_json::to_string(&response)?;
                tx.send(Message::Text(response_text.into())).await?;

                // If we sent a cancel or the job is terminal, close the connection
                if matches!(response, ServerMessage::Cancel) {
                    break;
                }
            },
            Message::Close(_) => {
                slog::info!(log, "WebSocket closed by client");
                break;
            },
            Message::Ping(data) => {
                tx.send(Message::Pong(data)).await?;
            },
            Message::Binary(_) | Message::Pong(_) | Message::Frame(_) => {
                // Ignore binary messages, pong responses, and raw frames
            },
        }
    }

    Ok(())
}

/// Handle a message from the runner and return the appropriate response.
async fn handle_runner_message(
    log: &slog::Logger,
    context: &ApiContext,
    job_id: JobId,
    msg: RunnerMessage,
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
        RunnerMessage::Completed { exit_code, output } => {
            slog::info!(log, "Job completed"; "job_id" => ?job_id, "exit_code" => exit_code);
            handle_completed(log, context, job_id, exit_code, output).await?;
        },
        RunnerMessage::Failed { exit_code, error } => {
            slog::warn!(log, "Job failed"; "job_id" => ?job_id, "exit_code" => ?exit_code, "error" => &error);
            handle_failed(log, context, job_id, exit_code).await?;
        },
    }

    Ok(ServerMessage::Ack)
}

/// Handle a Running message: transition job from Claimed to Running,
/// or update heartbeat if already Running (reconnection case).
async fn handle_running(
    log: &slog::Logger,
    context: &ApiContext,
    job_id: JobId,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let now = DateTime::now();

    let job: QueryJob = schema::job::table
        .filter(schema::job::id.eq(job_id))
        .first(write_conn!(context))
        .map_err(resource_not_found_err!(Job, job_id))?;

    match job.status {
        JobStatus::Running => {
            // Reconnection case: just update heartbeat
            slog::info!(log, "Runner reconnected to running job"; "job_id" => ?job_id);
            let update = UpdateJob {
                last_heartbeat: Some(Some(now)),
                modified: Some(now),
                ..Default::default()
            };
            diesel::update(schema::job::table.filter(schema::job::id.eq(job_id)))
                .set(&update)
                .execute(write_conn!(context))?;
        },
        JobStatus::Claimed => {
            // Normal transition: Claimed -> Running
            let update = UpdateJob {
                status: Some(JobStatus::Running),
                started: Some(Some(now)),
                last_heartbeat: Some(Some(now)),
                modified: Some(now),
                ..Default::default()
            };
            diesel::update(schema::job::table.filter(schema::job::id.eq(job_id)))
                .set(&update)
                .execute(write_conn!(context))?;

            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerJobUpdate(
                bencher_otel::JobStatusKind::Running,
            ));
        },
        JobStatus::Pending | JobStatus::Completed | JobStatus::Failed | JobStatus::Canceled => {
            slog::warn!(log, "Invalid state transition"; "job_id" => ?job_id, "from" => ?job.status, "to" => "running");
            return Err(format!(
                "Invalid state transition from {:?} to Running, expected Claimed or Running",
                job.status
            )
            .into());
        },
    }

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
    diesel::update(schema::job::table.filter(schema::job::id.eq(job_id)))
        .set(&update)
        .execute(write_conn!(context))?;

    // TODO: Billing logic - check elapsed minutes and bill to Stripe

    Ok(None)
}

/// Handle a Completed message: transition job from Running to Completed.
async fn handle_completed(
    log: &slog::Logger,
    context: &ApiContext,
    job_id: JobId,
    exit_code: i32,
    output: Option<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let now = DateTime::now();

    // Validate state transition: only Running -> Completed is valid
    let job: QueryJob = schema::job::table
        .filter(schema::job::id.eq(job_id))
        .first(write_conn!(context))
        .map_err(resource_not_found_err!(Job, job_id))?;

    if job.status != JobStatus::Running {
        slog::warn!(log, "Invalid state transition"; "job_id" => ?job_id, "from" => ?job.status, "to" => "completed");
        return Err(format!(
            "Invalid state transition from {:?} to Completed, expected Running",
            job.status
        )
        .into());
    }

    let update = UpdateJob {
        status: Some(JobStatus::Completed),
        completed: Some(Some(now)),
        exit_code: Some(Some(exit_code)),
        modified: Some(now),
        ..Default::default()
    };

    diesel::update(schema::job::table.filter(schema::job::id.eq(job_id)))
        .set(&update)
        .execute(write_conn!(context))?;

    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerJobUpdate(
        bencher_otel::JobStatusKind::Completed,
    ));

    // TODO: Store output somewhere (job table or separate results table)
    drop(output);

    Ok(())
}

/// Handle a Failed message: transition job from Claimed or Running to Failed.
async fn handle_failed(
    log: &slog::Logger,
    context: &ApiContext,
    job_id: JobId,
    exit_code: Option<i32>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let now = DateTime::now();

    // Validate state transition: Claimed -> Failed or Running -> Failed
    let job: QueryJob = schema::job::table
        .filter(schema::job::id.eq(job_id))
        .first(write_conn!(context))
        .map_err(resource_not_found_err!(Job, job_id))?;

    if !matches!(job.status, JobStatus::Claimed | JobStatus::Running) {
        slog::warn!(log, "Invalid state transition"; "job_id" => ?job_id, "from" => ?job.status, "to" => "failed");
        return Err(format!(
            "Invalid state transition from {:?} to Failed, expected Claimed or Running",
            job.status
        )
        .into());
    }

    let update = UpdateJob {
        status: Some(JobStatus::Failed),
        completed: Some(Some(now)),
        exit_code: Some(exit_code),
        modified: Some(now),
        ..Default::default()
    };

    diesel::update(schema::job::table.filter(schema::job::id.eq(job_id)))
        .set(&update)
        .execute(write_conn!(context))?;

    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::RunnerJobUpdate(
        bencher_otel::JobStatusKind::Failed,
    ));

    Ok(())
}
