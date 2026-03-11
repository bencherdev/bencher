//! Persistent WebSocket channel for runner job lifecycle.
//!
//! Single WebSocket connection that handles job assignment, execution status,
//! and stays open between jobs.

use bencher_json::{
    DEFAULT_POLL_TIMEOUT, RunnerResourceId,
    runner::{RunnerMessage, ServerMessage},
};
use bencher_schema::{
    auth_conn,
    context::ApiContext,
    model::runner::{QueryJob, job::spawn_heartbeat_timeout},
};
use dropshot::{Path, RequestContext, WebsocketChannelResult, WebsocketConnection, channel};
use futures::{SinkExt as _, StreamExt as _};
use schemars::JsonSchema;
use serde::Deserialize;
use std::time::Duration;

use tokio_tungstenite::tungstenite::{
    Message,
    protocol::{Role, WebSocketConfig},
};

use super::websocket::{ChannelError, handle_runner_message, handle_timeout};
use super::{POLL_INTERVAL, try_claim_job};
use crate::runner_token::RunnerToken;

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
        let Ok(poll_timeout) = wait_for_ready(&log, &mut tx, &mut rx).await else {
            break;
        };

        let deadline = tokio::time::Instant::now() + Duration::from_secs(u64::from(poll_timeout));

        // Poll for a job, checking for WS disconnect between polls
        let claimed_job =
            poll_for_job(&log, context, &runner_token, deadline, &mut tx, &mut rx).await;

        match claimed_job {
            Ok(Some(job)) => {
                // Send Job to runner
                let job_msg = ServerMessage::Job(Box::new(job.clone()));
                let text = serde_json::to_string(&job_msg)
                    .map_err(|e| dropshot::HttpError::for_internal_error(e.to_string()))?;
                if tx.send(Message::Text(text.into())).await.is_err() {
                    break;
                }

                // === EXECUTING STATE ===
                let job_db = QueryJob::from_uuid(auth_conn!(context), job.uuid)?;

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
                    .map_err(|e| dropshot::HttpError::for_internal_error(e.to_string()))?;
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
/// Ignores non-Ready messages with a warning. Returns an error on Close or disconnect.
async fn wait_for_ready<S, R>(
    log: &slog::Logger,
    tx: &mut S,
    rx: &mut R,
) -> Result<u32, ChannelError>
where
    S: futures::Sink<Message> + Unpin,
    R: futures::Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    loop {
        let Some(msg_result) = rx.next().await else {
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
) -> Result<Option<bencher_json::JsonClaimedJob>, ChannelError>
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
            Ok(Some(Ok(
                Message::Text(_) | Message::Binary(_) | Message::Pong(_) | Message::Frame(_),
            )))
            | Err(_) => {},
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

                let (response, close_reason) =
                    handle_runner_message(log, context, job, runner_msg).await?;

                // Reset heartbeat only on valid protocol messages
                last_heartbeat = tokio::time::Instant::now();

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
