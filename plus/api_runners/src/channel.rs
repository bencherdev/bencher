//! WebSocket channel for runner job execution.
//!
//! Provides a persistent connection for heartbeat and status updates during job execution.

use bencher_json::{DateTime, JobStatus, JobUuid, RunnerResourceId};
use bencher_schema::{
    auth_conn,
    context::ApiContext,
    error::{forbidden_error, resource_not_found_err},
    model::runner::{JobId, QueryJob, UpdateJob},
    schema, write_conn,
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::{Path, RequestContext, WebsocketConnection, WebsocketChannelResult, channel};
use futures::{SinkExt as _, StreamExt as _};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use dropshot::WebsocketConnectionRaw;
use tokio_tungstenite::tungstenite::{Message, protocol::Role};

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

    // Verify job exists and is claimed by this runner
    let job = QueryJob::from_uuid(auth_conn!(context), path_params.job)?;

    if job.runner_id != Some(runner_token.runner_id) {
        return Err(forbidden_error("Job is not assigned to this runner").into());
    }

    // Only allow channel for claimed jobs (not yet running)
    if job.status != JobStatus::Claimed {
        return Err(forbidden_error(format!(
            "Cannot open channel for job in {:?} status, expected Claimed",
            job.status
        ))
        .into());
    }

    let job_id = job.id;

    // Upgrade to WebSocket and handle messages
    let ws_stream = tokio_tungstenite::WebSocketStream::from_raw_socket(
        conn.into_inner(),
        Role::Server,
        None,
    )
    .await;

    handle_websocket(&log, context, job_id, ws_stream).await?;

    Ok(())
}

/// Handle WebSocket messages for a job.
async fn handle_websocket(
    log: &slog::Logger,
    context: &ApiContext,
    job_id: JobId,
    ws_stream: tokio_tungstenite::WebSocketStream<WebsocketConnectionRaw>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (mut tx, mut rx) = ws_stream.split();

    while let Some(msg_result) = rx.next().await {
        let msg = match msg_result {
            Ok(msg) => msg,
            Err(e) => {
                slog::warn!(log, "WebSocket error"; "error" => %e);
                break;
            }
        };

        match msg {
            Message::Text(text) => {
                let runner_msg: RunnerMessage = match serde_json::from_str(&text) {
                    Ok(msg) => msg,
                    Err(e) => {
                        slog::warn!(log, "Invalid message"; "error" => %e, "text" => text.to_string());
                        continue;
                    }
                };

                let response = handle_runner_message(log, context, job_id, runner_msg).await?;

                let response_text = serde_json::to_string(&response)?;
                tx.send(Message::Text(response_text.into())).await?;

                // If we sent a cancel or the job is terminal, close the connection
                if matches!(response, ServerMessage::Cancel) {
                    break;
                }
            }
            Message::Close(_) => {
                slog::info!(log, "WebSocket closed by client");
                break;
            }
            Message::Ping(data) => {
                tx.send(Message::Pong(data)).await?;
            }
            Message::Binary(_) | Message::Pong(_) | Message::Frame(_) => {
                // Ignore binary messages, pong responses, and raw frames
            }
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
    let now = DateTime::now();

    match msg {
        RunnerMessage::Running => {
            slog::info!(log, "Job running"; "job_id" => ?job_id);

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
        }
        RunnerMessage::Heartbeat => {
            slog::debug!(log, "Job heartbeat"; "job_id" => ?job_id);

            // Update last_heartbeat and check for billing
            let job: QueryJob = schema::job::table
                .filter(schema::job::id.eq(job_id))
                .first(auth_conn!(context))
                .map_err(resource_not_found_err!(Job, job_id))?;

            // Check if job was canceled
            if job.status == JobStatus::Canceled {
                return Ok(ServerMessage::Cancel);
            }

            let update = UpdateJob {
                last_heartbeat: Some(Some(now)),
                modified: Some(now),
                ..Default::default()
            };

            diesel::update(schema::job::table.filter(schema::job::id.eq(job_id)))
                .set(&update)
                .execute(write_conn!(context))?;

            // TODO: Billing logic - check elapsed minutes and bill to Stripe
        }
        RunnerMessage::Completed { exit_code, output } => {
            slog::info!(log, "Job completed"; "job_id" => ?job_id, "exit_code" => exit_code);

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
        }
        RunnerMessage::Failed { exit_code, error } => {
            slog::warn!(log, "Job failed"; "job_id" => ?job_id, "exit_code" => ?exit_code, "error" => &error);

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
        }
    }

    Ok(ServerMessage::Ack)
}
