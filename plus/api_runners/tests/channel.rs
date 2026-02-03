#![expect(unused_crate_dependencies, clippy::tests_outside_test_module)]
//! Integration tests for the WebSocket job channel endpoint.

mod common;

use bencher_api_tests::TestServer;
use bencher_json::{JobStatus, JobUuid, JsonJob, RunnerUuid};
use bencher_schema::schema;
use common::{create_runner, create_test_report, get_project_id, insert_test_job};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use futures::{SinkExt as _, StreamExt as _};
use http::StatusCode;
use serde::{Deserialize, Serialize};
use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest as _};

// ---- Message types matching the server definitions ----

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
enum RunnerMessage {
    Running,
    Heartbeat,
    Completed {
        exit_code: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        output: Option<String>,
    },
    Failed {
        #[serde(skip_serializing_if = "Option::is_none")]
        exit_code: Option<i32>,
        error: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
enum ServerMessage {
    Ack,
    Cancel,
}

type WsStream =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

// ---- Helper functions ----

/// Convert the `TestServer` HTTP URL to a WebSocket URL.
fn ws_url(server: &TestServer, path: &str) -> String {
    let http_url = server.api_url(path);
    http_url.replacen("http://", "ws://", 1)
}

/// Claim a pending job for a runner via the REST API.
#[expect(clippy::expect_used)]
async fn claim_job(server: &TestServer, runner_uuid: RunnerUuid, runner_token: &str) -> JsonJob {
    let body = serde_json::json!({ "poll_timeout": 5 });
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{runner_uuid}/jobs")))
        .header("Authorization", format!("Bearer {runner_token}"))
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let claimed: Option<JsonJob> = resp.json().await.expect("Failed to parse response");
    claimed.expect("Expected to claim a job")
}

/// Full setup: create user, org, project, runner, job, then claim the job.
/// Returns `(runner_uuid, runner_token, job_uuid)`.
async fn setup_claimed_job(server: &TestServer, suffix: &str) -> (RunnerUuid, String, JobUuid) {
    let admin = server
        .signup("Admin", &format!("ws-{suffix}@example.com"))
        .await;
    let org = server.create_org(&admin, &format!("Ws {suffix}")).await;
    let project = server
        .create_project(&admin, &org, &format!("Ws {suffix} proj"))
        .await;

    let runner = create_runner(server, &admin.token, &format!("Runner {suffix}")).await;
    let runner_token = runner.token.to_string();

    let project_id = get_project_id(server, project.slug.as_ref());
    let report_id = create_test_report(server, project_id);
    let job_uuid = insert_test_job(server, report_id);

    let claimed = claim_job(server, runner.uuid, &runner_token).await;
    assert_eq!(claimed.uuid, job_uuid);
    assert_eq!(claimed.status, JobStatus::Claimed);

    (runner.uuid, runner_token, job_uuid)
}

/// Build a WebSocket request with authorization header.
#[expect(clippy::expect_used)]
fn ws_request(
    server: &TestServer,
    runner_uuid: RunnerUuid,
    runner_token: &str,
    job_uuid: JobUuid,
) -> http::Request<()> {
    let url = ws_url(
        server,
        &format!("/v0/runners/{runner_uuid}/jobs/{job_uuid}/channel"),
    );
    let mut request = url.into_client_request().expect("Failed to build request");
    request.headers_mut().insert(
        "Authorization",
        format!("Bearer {runner_token}")
            .parse()
            .expect("Invalid header"),
    );
    request
}

/// Open an authenticated WebSocket connection to the job channel endpoint.
#[expect(clippy::expect_used)]
async fn connect_ws(
    server: &TestServer,
    runner_uuid: RunnerUuid,
    runner_token: &str,
    job_uuid: JobUuid,
) -> WsStream {
    let request = ws_request(server, runner_uuid, runner_token, job_uuid);
    let (ws_stream, _) = tokio_tungstenite::connect_async(request)
        .await
        .expect("Failed to connect WebSocket");
    ws_stream
}

/// Send a `RunnerMessage` over the WebSocket.
#[expect(clippy::expect_used)]
async fn send_msg(ws: &mut WsStream, msg: &RunnerMessage) {
    let text = serde_json::to_string(msg).expect("Failed to serialize");
    ws.send(Message::Text(text.into()))
        .await
        .expect("Failed to send message");
}

/// Receive and parse a `ServerMessage` from the WebSocket.
#[expect(clippy::expect_used)]
async fn recv_msg(ws: &mut WsStream) -> ServerMessage {
    let msg = ws.next().await.expect("Stream ended").expect("WS error");
    match msg {
        Message::Text(text) => serde_json::from_str(&text).expect("Failed to parse server message"),
        other => panic!("Expected text message, got: {other:?}"),
    }
}

/// Assert the WebSocket stream is closed (no more messages or Close frame).
///
/// Dropshot's `#[channel]` macro upgrades the WebSocket connection before the
/// handler runs. When the handler returns an error (auth failure, wrong state),
/// the connection is reset without a proper close handshake, which manifests as
/// `Some(Err(_))` rather than `None` or `Some(Ok(Message::Close(_)))`.
async fn assert_ws_closed(ws: &mut WsStream) {
    // Give the server a moment to close the connection
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    match ws.next().await {
        None => {},                        // Stream ended
        Some(Ok(Message::Close(_))) => {}, // Explicit close frame
        Some(Err(_)) => {},                // Connection reset (e.g. handler error)
        Some(Ok(other)) => panic!("Expected stream to be closed, got: {other:?}"),
    }
}

/// Read the job status directly from the database.
#[expect(clippy::expect_used)]
fn get_job_status(server: &TestServer, job_uuid: JobUuid) -> JobStatus {
    let mut conn = server.db_conn();
    schema::job::table
        .filter(schema::job::uuid.eq(job_uuid))
        .select(schema::job::status)
        .first(&mut conn)
        .expect("Failed to get job status")
}

/// Set the job status directly in the database (for testing cancellation).
#[expect(clippy::expect_used)]
fn set_job_status(server: &TestServer, job_uuid: JobUuid, status: JobStatus) {
    let mut conn = server.db_conn();
    diesel::update(schema::job::table.filter(schema::job::uuid.eq(job_uuid)))
        .set(schema::job::status.eq(status))
        .execute(&mut conn)
        .expect("Failed to set job status");
}

// =============================================================================
// Authentication / Pre-condition Tests
// =============================================================================

/// Connect with an invalid runner token. The server upgrades the WebSocket
/// (dropshot's `#[channel]` macro upgrades before the handler runs),
/// then the handler returns an error and the connection is closed.
#[tokio::test]
async fn test_channel_invalid_token() {
    let server = TestServer::new().await;
    let (runner_uuid, _runner_token, job_uuid) = setup_claimed_job(&server, "badtok").await;

    let url = ws_url(
        &server,
        &format!("/v0/runners/{runner_uuid}/jobs/{job_uuid}/channel"),
    );
    let mut request = url.into_client_request().expect("Failed to build request");
    request.headers_mut().insert(
        "Authorization",
        "Bearer bencher_runner_badbadbadbad"
            .parse()
            .expect("Invalid header"),
    );

    // The WebSocket upgrade may succeed (dropshot upgrades before handler runs),
    // but the connection should immediately close.
    match tokio_tungstenite::connect_async(request).await {
        Err(_) => {}, // Connection rejected at HTTP level, expected
        Ok((mut ws, _)) => {
            // Connection upgraded, but handler should close it immediately
            assert_ws_closed(&mut ws).await;
        },
    }
}

/// Connect as a different runner that doesn't own the job.
#[tokio::test]
async fn test_channel_wrong_runner() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "ws-wrongrun@example.com").await;
    let org = server.create_org(&admin, "Ws wrongrun").await;
    let project = server
        .create_project(&admin, &org, "Ws wrongrun proj")
        .await;

    let runner1 = create_runner(&server, &admin.token, "Runner one").await;
    let runner1_token = runner1.token.to_string();
    let runner2 = create_runner(&server, &admin.token, "Runner two").await;
    let runner2_token = runner2.token.to_string();

    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let job_uuid = insert_test_job(&server, report_id);
    let _claimed = claim_job(&server, runner1.uuid, &runner1_token).await;

    // Try to open channel with runner2 (doesn't own the job)
    let request = ws_request(&server, runner2.uuid, &runner2_token, job_uuid);
    match tokio_tungstenite::connect_async(request).await {
        Err(_) => {},
        Ok((mut ws, _)) => {
            assert_ws_closed(&mut ws).await;
        },
    }
}

/// Connect to a pending job (not yet claimed).
#[tokio::test]
async fn test_channel_job_not_claimed() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "ws-notclaimed@example.com").await;
    let org = server.create_org(&admin, "Ws notclaimed").await;
    let project = server
        .create_project(&admin, &org, "Ws notclaimed proj")
        .await;

    let runner = create_runner(&server, &admin.token, "Runner notclaimed").await;
    let runner_token = runner.token.to_string();

    // Create a job but do NOT claim it (stays Pending)
    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let job_uuid = insert_test_job(&server, report_id);

    let request = ws_request(&server, runner.uuid, &runner_token, job_uuid);
    match tokio_tungstenite::connect_async(request).await {
        Err(_) => {},
        Ok((mut ws, _)) => {
            assert_ws_closed(&mut ws).await;
        },
    }
}

/// Connect to a job that is already running.
#[tokio::test]
async fn test_channel_job_already_running() {
    let server = TestServer::new().await;
    let (runner_uuid, runner_token, job_uuid) = setup_claimed_job(&server, "running").await;

    // Set job to Running directly in DB
    set_job_status(&server, job_uuid, JobStatus::Running);

    let request = ws_request(&server, runner_uuid, &runner_token, job_uuid);
    match tokio_tungstenite::connect_async(request).await {
        Err(_) => {},
        Ok((mut ws, _)) => {
            assert_ws_closed(&mut ws).await;
        },
    }
}

// =============================================================================
// Happy Path Tests
// =============================================================================

/// Full lifecycle: Running -> Heartbeat -> Completed.
#[tokio::test]
async fn test_channel_lifecycle_completed() {
    let server = TestServer::new().await;
    let (runner_uuid, runner_token, job_uuid) = setup_claimed_job(&server, "done").await;

    let mut ws = connect_ws(&server, runner_uuid, &runner_token, job_uuid).await;

    // Send Running
    send_msg(&mut ws, &RunnerMessage::Running).await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Running);

    // Send Heartbeat
    send_msg(&mut ws, &RunnerMessage::Heartbeat).await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));

    // Send Completed
    send_msg(
        &mut ws,
        &RunnerMessage::Completed {
            exit_code: 0,
            output: Some("benchmark results".to_owned()),
        },
    )
    .await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Completed);

    // Verify exit code in DB
    let mut conn = server.db_conn();
    let exit_code: Option<i32> = schema::job::table
        .filter(schema::job::uuid.eq(job_uuid))
        .select(schema::job::exit_code)
        .first(&mut conn)
        .expect("Failed to get exit code");
    assert_eq!(exit_code, Some(0));

    ws.close(None).await.expect("Failed to close WebSocket");
}

/// Full lifecycle: Running -> Failed.
#[tokio::test]
async fn test_channel_lifecycle_failed() {
    let server = TestServer::new().await;
    let (runner_uuid, runner_token, job_uuid) = setup_claimed_job(&server, "fail").await;

    let mut ws = connect_ws(&server, runner_uuid, &runner_token, job_uuid).await;

    // Send Running
    send_msg(&mut ws, &RunnerMessage::Running).await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Running);

    // Send Failed
    send_msg(
        &mut ws,
        &RunnerMessage::Failed {
            exit_code: Some(1),
            error: "segfault".to_owned(),
        },
    )
    .await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Failed);

    // Verify exit code in DB
    let mut conn = server.db_conn();
    let exit_code: Option<i32> = schema::job::table
        .filter(schema::job::uuid.eq(job_uuid))
        .select(schema::job::exit_code)
        .first(&mut conn)
        .expect("Failed to get exit code");
    assert_eq!(exit_code, Some(1));

    ws.close(None).await.expect("Failed to close WebSocket");
}

// =============================================================================
// Cancellation Test
// =============================================================================

/// Heartbeat detects a canceled job and receives Cancel.
#[tokio::test]
async fn test_channel_heartbeat_cancel() {
    let server = TestServer::new().await;
    let (runner_uuid, runner_token, job_uuid) = setup_claimed_job(&server, "cancel").await;

    let mut ws = connect_ws(&server, runner_uuid, &runner_token, job_uuid).await;

    // Send Running
    send_msg(&mut ws, &RunnerMessage::Running).await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));

    // Cancel the job directly in DB (simulating user cancellation)
    set_job_status(&server, job_uuid, JobStatus::Canceled);

    // Next heartbeat should detect cancellation
    send_msg(&mut ws, &RunnerMessage::Heartbeat).await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Cancel));

    // Server closes the connection after sending Cancel
    assert_ws_closed(&mut ws).await;
}

// =============================================================================
// Message Handling Tests
// =============================================================================

/// Invalid JSON text is ignored; connection stays open for valid messages.
#[tokio::test]
async fn test_channel_invalid_json() {
    let server = TestServer::new().await;
    let (runner_uuid, runner_token, job_uuid) = setup_claimed_job(&server, "badjson").await;

    let mut ws = connect_ws(&server, runner_uuid, &runner_token, job_uuid).await;

    // Send invalid JSON - server should ignore it
    ws.send(Message::Text("not valid json{{{".into()))
        .await
        .expect("Failed to send");

    // Connection should still be open; send a valid Running message
    send_msg(&mut ws, &RunnerMessage::Running).await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Running);

    ws.close(None).await.expect("Failed to close WebSocket");
}

/// Ping frame receives a Pong response.
#[tokio::test]
async fn test_channel_ping_pong() {
    let server = TestServer::new().await;
    let (runner_uuid, runner_token, job_uuid) = setup_claimed_job(&server, "pong").await;

    let mut ws = connect_ws(&server, runner_uuid, &runner_token, job_uuid).await;

    // Send Ping
    let ping_data = b"hello".to_vec();
    ws.send(Message::Ping(ping_data.clone().into()))
        .await
        .expect("Failed to send ping");

    // Should receive Pong with same data
    let msg = ws.next().await.expect("Stream ended").expect("WS error");
    assert!(
        matches!(&msg, Message::Pong(data) if data.as_ref() == ping_data.as_slice()),
        "Expected Pong with matching data, got: {msg:?}"
    );

    ws.close(None).await.expect("Failed to close WebSocket");
}
