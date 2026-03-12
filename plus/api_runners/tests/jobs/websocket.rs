use super::common::{
    WsStream, assert_ws_closed, associate_runner_spec, connect_channel_ws as connect_channel,
    create_runner, create_test_report, get_project_id, get_runner_id, insert_test_job,
    insert_test_job_with_optional_fields, insert_test_job_with_timeout, insert_test_spec,
    recv_server_msg as recv_msg, send_runner_msg as send_msg, set_job_runner_id, set_job_status,
    ws_url,
};
use api_runners::{RunnerMessage, ServerMessage};
use bencher_api_tests::TestServer;
use bencher_json::{
    JobStatus, JobUuid, JsonRunnerToken, PollTimeout, RunnerUuid, runner::JsonIterationOutput,
};
use bencher_schema::schema;
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use futures::{SinkExt as _, StreamExt as _};
use http::StatusCode;
use tokio_tungstenite::tungstenite::{
    Message, client::IntoClientRequest as _, protocol::WebSocketConfig,
};

/// Full setup: create user, org, project, runner, job, then connect channel,
/// send Ready, and receive Job.
/// Returns `(ws, runner_uuid, runner_token, job_uuid)`.
#[expect(clippy::expect_used, clippy::panic)]
async fn setup_claimed_job(
    server: &TestServer,
    suffix: &str,
) -> (WsStream, RunnerUuid, String, JobUuid) {
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
    let (_, spec_id) = insert_test_spec(server);
    let job_uuid = insert_test_job(server, report_id, spec_id);

    let runner_id = get_runner_id(server, runner.uuid);
    associate_runner_spec(server, runner_id, spec_id);

    // Connect channel, send Ready, receive Job
    let mut ws = connect_channel(server, runner.uuid, &runner_token).await;
    let ready = RunnerMessage::Ready {
        poll_timeout: Some(PollTimeout::try_from(5).expect("Invalid poll timeout")),
    };
    send_msg(&mut ws, &ready).await;
    let response = recv_msg(&mut ws).await;
    match response {
        ServerMessage::Job(job) => {
            assert_eq!(job.uuid, job_uuid, "Claimed job UUID should match");
        },
        ServerMessage::Ack | ServerMessage::NoJob | ServerMessage::Cancel => {
            panic!("Expected Job message, got: {response:?}");
        },
    }

    (ws, runner.uuid, runner_token, job_uuid)
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

// =============================================================================
// Authentication / Pre-condition Tests
// =============================================================================

/// Connect with an invalid runner token. The server upgrades the WebSocket
/// (dropshot's `#[channel]` macro upgrades before the handler runs),
/// then the handler returns an error and the connection is closed.
#[tokio::test]
async fn channel_invalid_token() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "ws-badtok@example.com").await;

    let runner = create_runner(&server, &admin.token, "Runner badtok").await;

    let url = ws_url(&server, &format!("/v0/runners/{}/channel", runner.uuid));
    let mut request = url.into_client_request().expect("Failed to build request");
    request.headers_mut().insert(
        bencher_json::AUTHORIZATION,
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

/// Connect as a different runner (wrong token for the runner UUID).
#[tokio::test]
async fn channel_wrong_runner() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "ws-wrongrun@example.com").await;

    let runner1 = create_runner(&server, &admin.token, "Runner one").await;
    let runner2 = create_runner(&server, &admin.token, "Runner two").await;
    let runner1_token: &str = runner1.token.as_ref();

    // Try to connect to runner2's channel using runner1's token
    let url = ws_url(&server, &format!("/v0/runners/{}/channel", runner2.uuid));
    let mut request = url.into_client_request().expect("Failed to build request");
    request.headers_mut().insert(
        bencher_json::AUTHORIZATION,
        bencher_json::bearer_header(runner1_token)
            .parse()
            .expect("Invalid header"),
    );

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

/// Full lifecycle: Ready -> Job -> Running -> Heartbeat -> Completed.
/// After Completed, server sends Ack and connection stays open.
#[tokio::test]
async fn channel_lifecycle_completed() {
    let server = TestServer::new().await;
    let (mut ws, _runner_uuid, _runner_token, job_uuid) = setup_claimed_job(&server, "done").await;

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
            job: job_uuid,
            results: vec![JsonIterationOutput {
                exit_code: 0,
                stdout: None,
                stderr: None,
                output: None,
            }],
        },
    )
    .await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Processed);

    // Connection stays open (no close frame from server).
    // Close from client side.
    ws.close(None).await.expect("Failed to close WebSocket");
}

/// Full lifecycle: Ready -> Job -> Running -> Failed.
/// After Failed, server sends Ack and connection stays open.
#[tokio::test]
async fn channel_lifecycle_failed() {
    let server = TestServer::new().await;
    let (mut ws, _runner_uuid, _runner_token, job_uuid) = setup_claimed_job(&server, "fail").await;

    // Send Running
    send_msg(&mut ws, &RunnerMessage::Running).await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Running);

    // Send Failed
    send_msg(
        &mut ws,
        &RunnerMessage::Failed {
            job: job_uuid,
            results: vec![JsonIterationOutput {
                exit_code: 1,
                stdout: None,
                stderr: None,
                output: None,
            }],
            error: "segfault".to_owned(),
        },
    )
    .await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Failed);

    // Connection stays open. Close from client side.
    ws.close(None).await.expect("Failed to close WebSocket");
}

// =============================================================================
// Cancellation Tests
// =============================================================================

/// Heartbeat detects a canceled job and receives Cancel.
/// In the channel model, after Cancel the connection stays open.
#[tokio::test]
async fn channel_heartbeat_cancel() {
    let server = TestServer::new().await;
    let (mut ws, _runner_uuid, _runner_token, job_uuid) =
        setup_claimed_job(&server, "cancel").await;

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

    // In the channel model, terminal messages (including Cancel) cause the
    // execute_loop to return JobDone, transitioning back to Idle state.
    // The connection stays open. Close from client side.
    ws.close(None).await.expect("Failed to close WebSocket");
}

/// When a job is canceled while a runner has an active channel,
/// the runner receives `ServerMessage::Cancel` on the next heartbeat and
/// can acknowledge with `RunnerMessage::Canceled`, which gets Ack.
/// Connection stays open for next job cycle.
#[tokio::test]
async fn channel_canceled_message_over_ws() {
    let server = TestServer::new().await;
    let (mut ws, _runner_uuid, _runner_token, job_uuid) =
        setup_claimed_job(&server, "canceled-ws").await;

    // Transition to Running
    send_msg(&mut ws, &RunnerMessage::Running).await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Running);

    // Cancel the job directly in DB (simulating user/admin cancellation)
    set_job_status(&server, job_uuid, JobStatus::Canceled);

    // Next heartbeat should detect cancellation and return Cancel
    send_msg(&mut ws, &RunnerMessage::Heartbeat).await;
    let resp = recv_msg(&mut ws).await;
    assert!(
        matches!(resp, ServerMessage::Cancel),
        "Expected Cancel message after job cancellation, got: {resp:?}"
    );

    // Verify job remains in Canceled state
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Canceled);

    // Connection stays open. Close from client side.
    ws.close(None).await.expect("Failed to close WebSocket");
}

/// Runner sends Canceled (e.g., it detected the cancel signal itself).
/// Server sends Ack, connection stays open.
#[tokio::test]
async fn channel_runner_sends_canceled() {
    let server = TestServer::new().await;
    let (mut ws, _runner_uuid, _runner_token, job_uuid) =
        setup_claimed_job(&server, "runner-canceled").await;

    // Transition to Running
    send_msg(&mut ws, &RunnerMessage::Running).await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));

    // Runner sends Canceled (e.g., it detected the cancel signal itself)
    send_msg(&mut ws, &RunnerMessage::Canceled { job: job_uuid }).await;
    let resp = recv_msg(&mut ws).await;
    assert!(
        matches!(resp, ServerMessage::Ack),
        "Expected Ack for Canceled message, got: {resp:?}"
    );

    // Job should be in Canceled state
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Canceled);

    // Connection stays open. Close from client side.
    ws.close(None).await.expect("Failed to close WebSocket");
}

/// Verify that a runner can acknowledge cancellation after receiving Cancel.
/// 1. Set up a running job, cancel it in DB
/// 2. Send Heartbeat on the channel -> get Cancel
/// 3. Verify job is Canceled
#[tokio::test]
async fn channel_canceled_acknowledgment() {
    let server = TestServer::new().await;
    let (mut ws, _runner_uuid, _runner_token, job_uuid) =
        setup_claimed_job(&server, "cancel-ack").await;

    // Send Running
    send_msg(&mut ws, &RunnerMessage::Running).await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Running);

    // Cancel the job in DB (simulating user/admin cancellation)
    set_job_status(&server, job_uuid, JobStatus::Canceled);

    // Heartbeat should detect cancellation
    send_msg(&mut ws, &RunnerMessage::Heartbeat).await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Cancel));

    // Verify the job is in Canceled state
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Canceled);

    // Connection stays open. Close from client side.
    ws.close(None).await.expect("Failed to close WebSocket");
}

// =============================================================================
// Message Handling Tests
// =============================================================================

/// Invalid JSON text is ignored; connection stays open for valid messages.
#[tokio::test]
async fn channel_invalid_json() {
    let server = TestServer::new().await;
    let (mut ws, _runner_uuid, _runner_token, job_uuid) =
        setup_claimed_job(&server, "badjson").await;

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

/// Heartbeat timeout: open channel, claim job, send Running, then go silent.
/// The server should mark the job as Failed after the heartbeat timeout.
/// Uses tokio time manipulation to avoid waiting real wall-clock time.
#[tokio::test]
#[expect(clippy::panic)]
async fn channel_heartbeat_timeout() {
    let server = TestServer::new().await;
    let (mut ws, _runner_uuid, _runner_token, job_uuid) =
        setup_claimed_job(&server, "hbtimeout").await;

    // Send Running to start the job
    send_msg(&mut ws, &RunnerMessage::Running).await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Running);

    // Pause tokio time and advance past the heartbeat timeout (5s in tests).
    // advance() fires all pending timers and processes resulting tasks, so the
    // server's timeout handler runs during the advance — marking the job as
    // Failed and closing the connection — without any real wall-clock wait.
    tokio::time::pause();
    tokio::time::advance(std::time::Duration::from_secs(6)).await;
    tokio::time::resume();

    // The server's heartbeat timeout should have fired and the WS stream ends.
    match ws.next().await {
        None | Some(Ok(Message::Close(_)) | Err(_)) => {
            // Connection closed as expected
        },
        Some(Ok(other)) => {
            panic!("Expected connection to close from timeout, got: {other:?}");
        },
    }

    // Verify job is marked as Failed (already done inline by the WS timeout handler)
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Failed);
}

/// Ping frame receives a Pong response.
#[tokio::test]
async fn channel_ping_pong() {
    let server = TestServer::new().await;
    let (mut ws, _runner_uuid, _runner_token, _job_uuid) = setup_claimed_job(&server, "pong").await;

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

// =============================================================================
// Job Spec Handling Tests
// =============================================================================

/// Full lifecycle with a job containing optional spec fields (entrypoint, cmd, env).
/// Verifies that jobs with complete specs work correctly through the channel.
#[tokio::test]
#[expect(clippy::panic)]
async fn channel_lifecycle_with_full_spec() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "ws-fullspec@example.com").await;
    let org = server.create_org(&admin, "Ws fullspec").await;
    let project = server
        .create_project(&admin, &org, "Ws fullspec proj")
        .await;

    let runner = create_runner(&server, &admin.token, "Runner fullspec").await;
    let runner_token = runner.token.to_string();

    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let (_, spec_id) = insert_test_spec(&server);
    // Use the helper that creates a job with optional fields populated
    let job_uuid = insert_test_job_with_optional_fields(&server, report_id, project.uuid, spec_id);

    let runner_id = get_runner_id(&server, runner.uuid);
    associate_runner_spec(&server, runner_id, spec_id);

    // Connect channel, send Ready, receive Job
    let mut ws = connect_channel(&server, runner.uuid, &runner_token).await;
    let ready = RunnerMessage::Ready {
        poll_timeout: Some(PollTimeout::try_from(5).expect("Invalid poll timeout")),
    };
    send_msg(&mut ws, &ready).await;
    let response = recv_msg(&mut ws).await;
    let claimed = match response {
        ServerMessage::Job(job) => {
            assert_eq!(job.uuid, job_uuid);
            *job
        },
        ServerMessage::Ack | ServerMessage::NoJob | ServerMessage::Cancel => {
            panic!("Expected Job message, got: {response:?}");
        },
    };

    // Verify the config has optional fields
    let config = &claimed.config;
    assert!(config.entrypoint.is_some());
    assert!(config.cmd.is_some());
    assert!(config.env.is_some());

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
            job: job_uuid,
            results: vec![JsonIterationOutput {
                exit_code: 0,
                stdout: None,
                stderr: None,
                output: None,
            }],
        },
    )
    .await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Processed);

    ws.close(None).await.expect("Failed to close WebSocket");
}

// =============================================================================
// Large Message Test
// =============================================================================

/// Send a message that exceeds the server's WebSocket `max_message_size`.
/// Uses a server with a 1 MiB body limit so the 2 MiB payload is rejected
/// at the protocol level and the connection is closed immediately.
#[tokio::test]
#[expect(clippy::panic)]
async fn channel_large_message() {
    // 1 MiB body limit so the WebSocket max_message_size < our 2 MiB payload
    let server = TestServer::new_with_limits(30, 1024 * 1024).await;
    let (mut ws, runner_uuid, runner_token, _job_uuid) =
        setup_claimed_job(&server, "largemsg").await;
    // Close the setup connection; we need a custom config for large frames
    ws.close(None).await.expect("Failed to close setup WS");

    // Reconnect with a custom config that allows 3 MiB client-side
    let url = ws_url(&server, &format!("/v0/runners/{runner_uuid}/channel"));
    let mut request = url.into_client_request().expect("Failed to build request");
    request.headers_mut().insert(
        bencher_json::AUTHORIZATION,
        bencher_json::bearer_header(&runner_token)
            .parse()
            .expect("Invalid header"),
    );

    let mut config = WebSocketConfig::default();
    config.max_message_size = Some(3 * 1024 * 1024);
    config.max_frame_size = Some(3 * 1024 * 1024);

    let (mut ws, _) = tokio_tungstenite::connect_async_with_config(request, Some(config), false)
        .await
        .expect("Failed to connect WebSocket");

    // Build a 2 MiB text payload (exceeds the server's 1 MiB limit)
    let payload = "x".repeat(2 * 1024 * 1024);
    let result = ws.send(Message::Text(payload.into())).await;

    // The send may succeed from the client's perspective (buffered),
    // but the server will reject it and close the connection.
    if result.is_ok() {
        // Wait for the server to close the connection
        match tokio::time::timeout(std::time::Duration::from_secs(5), ws.next()).await {
            Ok(None | Some(Ok(Message::Close(_)) | Err(_))) => {
                // Connection closed as expected
            },
            Ok(Some(Ok(other))) => {
                panic!("Expected connection to close after oversized message, got: {other:?}");
            },
            Err(elapsed) => {
                panic!("Timed out waiting for server to close connection: {elapsed}");
            },
        }
    }
    // If send itself failed, that's also acceptable (connection already closing)
}

// =============================================================================
// Heartbeat Timeout Precision Tests
// =============================================================================

/// Verify that Ping frames do NOT reset the heartbeat timeout.
/// Send Running (valid), then only send Ping frames. The job should eventually
/// be marked Failed because Ping does NOT count as a valid heartbeat message.
/// Uses tokio time manipulation: pause after Running, advance past the timeout.
#[tokio::test]
#[expect(clippy::panic)]
async fn channel_ping_does_not_reset_heartbeat_timeout() {
    let server = TestServer::new().await;
    let (mut ws, _runner_uuid, _runner_token, job_uuid) =
        setup_claimed_job(&server, "ping-no-reset").await;

    // Send Running to start the job and reset the heartbeat clock
    send_msg(&mut ws, &RunnerMessage::Running).await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Running);

    // Send a Ping frame — should NOT reset heartbeat timeout
    ws.send(Message::Ping(b"keep-alive".to_vec().into()))
        .await
        .expect("Failed to send ping");
    // Consume the Pong response
    let pong = ws.next().await.expect("Stream ended").expect("WS error");
    assert!(matches!(pong, Message::Pong(_)));

    // Pause tokio time and advance past the heartbeat timeout (5s in tests).
    // The Ping above should NOT have reset the heartbeat clock, so advancing 6s
    // past the last valid message (Running) should trigger the timeout.
    tokio::time::pause();
    tokio::time::advance(std::time::Duration::from_secs(6)).await;
    tokio::time::resume();

    // The heartbeat timeout should have fired — connection should close
    match ws.next().await {
        None | Some(Ok(Message::Close(_)) | Err(_)) => {
            // Connection closed as expected
        },
        Some(Ok(other)) => {
            panic!("Expected connection to close from timeout, got: {other:?}");
        },
    }

    // Job should be Failed because no valid protocol message was sent after Running
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Failed);
}

// =============================================================================
// Binary Message Test
// =============================================================================

/// Binary WebSocket messages should be ignored; the connection stays open.
#[tokio::test]
async fn channel_binary_message() {
    let server = TestServer::new().await;
    let (mut ws, _runner_uuid, _runner_token, job_uuid) =
        setup_claimed_job(&server, "binary").await;

    // Send a binary message — server should ignore it
    ws.send(Message::Binary(b"\x00\x01\x02\x03".to_vec().into()))
        .await
        .expect("Failed to send binary message");

    // Connection should still be open; send a valid Running message
    send_msg(&mut ws, &RunnerMessage::Running).await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Running);

    // Send another binary message to be sure
    ws.send(Message::Binary(vec![0xff; 100].into()))
        .await
        .expect("Failed to send second binary message");

    // Send a Heartbeat to verify connection is still functional
    send_msg(&mut ws, &RunnerMessage::Heartbeat).await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));

    ws.close(None).await.expect("Failed to close WebSocket");
}

// =============================================================================
// Status Transition Edge Cases
// =============================================================================

/// Completed sent before Running (job is still Claimed) should be rejected.
/// The server requires Running status for a Completed transition.
#[tokio::test]
async fn channel_completed_before_running() {
    let server = TestServer::new().await;
    let (mut ws, _runner_uuid, _runner_token, job_uuid) =
        setup_claimed_job(&server, "complete-early").await;

    // Send Completed without first sending Running (job is still Claimed)
    send_msg(
        &mut ws,
        &RunnerMessage::Completed {
            job: job_uuid,
            results: vec![JsonIterationOutput {
                exit_code: 0,
                stdout: None,
                stderr: None,
                output: None,
            }],
        },
    )
    .await;

    // The server should reject this (invalid state transition) and close
    assert_ws_closed(&mut ws).await;

    // Job should still be Claimed (not Completed) since the transition was invalid
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Claimed);
}

/// Failed sent from Claimed state (before Running) should succeed.
/// The server allows Failed from both Claimed and Running states.
#[tokio::test]
async fn channel_failed_from_claimed() {
    let server = TestServer::new().await;
    let (mut ws, _runner_uuid, _runner_token, job_uuid) =
        setup_claimed_job(&server, "fail-early").await;

    // Send Failed without first sending Running (job is still Claimed)
    send_msg(
        &mut ws,
        &RunnerMessage::Failed {
            job: job_uuid,
            results: vec![JsonIterationOutput {
                exit_code: 127,
                stdout: None,
                stderr: None,
                output: None,
            }],
            error: "command not found".to_owned(),
        },
    )
    .await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));

    // Job should be Failed — transition from Claimed is allowed
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Failed);

    // Connection stays open. Close from client side.
    ws.close(None).await.expect("Failed to close WebSocket");
}

// =============================================================================
// Terminal Messages — Ack Without Close
// =============================================================================

/// Server sends Ack after Completed but does NOT close the connection.
/// Verify Ack received, then close from client side.
#[tokio::test]
async fn channel_completed_ack_no_close() {
    let server = TestServer::new().await;
    let (mut ws, _runner_uuid, _runner_token, job_uuid) =
        setup_claimed_job(&server, "close-done").await;

    // Send Running
    send_msg(&mut ws, &RunnerMessage::Running).await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));

    // Send Completed
    send_msg(
        &mut ws,
        &RunnerMessage::Completed {
            job: job_uuid,
            results: vec![JsonIterationOutput {
                exit_code: 0,
                stdout: None,
                stderr: None,
                output: None,
            }],
        },
    )
    .await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Processed);

    // Connection stays open (no close frame from server). Close from client side.
    ws.close(None).await.expect("Failed to close WebSocket");
}

/// Server sends Ack after Failed but does NOT close the connection.
#[tokio::test]
async fn channel_failed_ack_no_close() {
    let server = TestServer::new().await;
    let (mut ws, _runner_uuid, _runner_token, job_uuid) =
        setup_claimed_job(&server, "close-fail").await;

    // Send Running
    send_msg(&mut ws, &RunnerMessage::Running).await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));

    // Send Failed
    send_msg(
        &mut ws,
        &RunnerMessage::Failed {
            job: job_uuid,
            results: vec![JsonIterationOutput {
                exit_code: 1,
                stdout: None,
                stderr: None,
                output: None,
            }],
            error: "test failure".to_owned(),
        },
    )
    .await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Failed);

    // Connection stays open. Close from client side.
    ws.close(None).await.expect("Failed to close WebSocket");
}

/// Server sends Ack after Canceled but does NOT close the connection.
#[tokio::test]
async fn channel_canceled_ack_no_close() {
    let server = TestServer::new().await;
    let (mut ws, _runner_uuid, _runner_token, job_uuid) =
        setup_claimed_job(&server, "close-cancel").await;

    // Send Running
    send_msg(&mut ws, &RunnerMessage::Running).await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));

    // Runner sends Canceled
    send_msg(&mut ws, &RunnerMessage::Canceled { job: job_uuid }).await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Canceled);

    // Connection stays open. Close from client side.
    ws.close(None).await.expect("Failed to close WebSocket");
}

// =============================================================================
// WebSocket Output Tests
// =============================================================================

/// Completed with stdout/stderr/output fields.
#[tokio::test]
async fn channel_completed_with_output() {
    let server = TestServer::new().await;
    let (mut ws, _runner_uuid, _runner_token, job_uuid) =
        setup_claimed_job(&server, "output-done").await;

    // Send Running
    send_msg(&mut ws, &RunnerMessage::Running).await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));

    // Send Completed with stdout and output
    let mut output = std::collections::BTreeMap::new();
    output.insert(
        camino::Utf8PathBuf::from("/output/results.json"),
        "final results".to_owned(),
    );
    send_msg(
        &mut ws,
        &RunnerMessage::Completed {
            job: job_uuid,
            results: vec![JsonIterationOutput {
                exit_code: 0,
                stdout: Some("line of output\n".into()),
                stderr: None,
                output: Some(output),
            }],
        },
    )
    .await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));
    // File output ("final results") is now passed to the adapter (no longer silently dropped).
    // The Magic adapter cannot parse it, so process_results fails and the job is marked Failed.
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Failed);

    // Connection stays open. Close from client side.
    ws.close(None).await.expect("Failed to close WebSocket");
}

/// Failed with stderr and output fields.
#[tokio::test]
async fn channel_failed_with_output() {
    let server = TestServer::new().await;
    let (mut ws, _runner_uuid, _runner_token, job_uuid) =
        setup_claimed_job(&server, "output-fail").await;

    // Send Running
    send_msg(&mut ws, &RunnerMessage::Running).await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));

    // Send Failed with stderr
    send_msg(
        &mut ws,
        &RunnerMessage::Failed {
            job: job_uuid,
            results: vec![JsonIterationOutput {
                exit_code: 1,
                stdout: Some("partial output\n".into()),
                stderr: Some("error output\n".into()),
                output: None,
            }],
            error: "benchmark crashed".to_owned(),
        },
    )
    .await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Failed);

    // Connection stays open. Close from client side.
    ws.close(None).await.expect("Failed to close WebSocket");
}

/// Output message with stderr only.
#[tokio::test]
async fn channel_completed_with_stderr_only() {
    let server = TestServer::new().await;
    let (mut ws, _runner_uuid, _runner_token, job_uuid) =
        setup_claimed_job(&server, "stderr-only").await;

    // Send Running
    send_msg(&mut ws, &RunnerMessage::Running).await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));

    // Send Completed with only stderr (e.g., benchmark wrote to stderr)
    send_msg(
        &mut ws,
        &RunnerMessage::Completed {
            job: job_uuid,
            results: vec![JsonIterationOutput {
                exit_code: 0,
                stdout: None,
                stderr: Some("warning: benchmark variance high\n".into()),
                output: None,
            }],
        },
    )
    .await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Processed);

    // Connection stays open. Close from client side.
    ws.close(None).await.expect("Failed to close WebSocket");
}

/// Verifies job transitions to Failed (not Processed) when `process_results` fails.
#[tokio::test]
async fn channel_completed_result_processing_failure() {
    let server = TestServer::new().await;
    let (mut ws, _runner_uuid, _runner_token, job_uuid) =
        setup_claimed_job(&server, "proc-fail").await;

    // Send Running
    send_msg(&mut ws, &RunnerMessage::Running).await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));

    // Send Completed with stdout that the adapter cannot parse.
    // The Magic adapter will fail to convert this invalid benchmark output.
    send_msg(
        &mut ws,
        &RunnerMessage::Completed {
            job: job_uuid,
            results: vec![JsonIterationOutput {
                exit_code: 0,
                stdout: Some("this is not valid benchmark output".into()),
                stderr: None,
                output: None,
            }],
        },
    )
    .await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));

    // Job is marked Failed (not Processed) because process_results failed
    // (the adapter could not parse the output).
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Failed);

    // Connection stays open. Close from client side.
    ws.close(None).await.expect("Failed to close WebSocket");
}

// =============================================================================
// Multi-Iteration Tests
// =============================================================================

/// Completed with multiple iterations (empty stdout/output, `exit_code` 0).
/// Since there's no stdout or file output, the adapter receives an empty results
/// array, which succeeds and the job transitions to Processed.
#[tokio::test]
async fn channel_completed_multiple_iterations() {
    let server = TestServer::new().await;
    let (mut ws, _runner_uuid, _runner_token, job_uuid) =
        setup_claimed_job(&server, "multi-iter").await;

    // Send Running
    send_msg(&mut ws, &RunnerMessage::Running).await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Running);

    // Send Completed with 3 iterations, all empty
    send_msg(
        &mut ws,
        &RunnerMessage::Completed {
            job: job_uuid,
            results: vec![
                JsonIterationOutput {
                    exit_code: 0,
                    stdout: None,
                    stderr: None,
                    output: None,
                },
                JsonIterationOutput {
                    exit_code: 0,
                    stdout: None,
                    stderr: None,
                    output: None,
                },
                JsonIterationOutput {
                    exit_code: 0,
                    stdout: None,
                    stderr: None,
                    output: None,
                },
            ],
        },
    )
    .await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));
    // Empty results -> adapter succeeds (no benchmarks to parse) -> Processed
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Processed);

    // Connection stays open. Close from client side.
    ws.close(None).await.expect("Failed to close WebSocket");
}

/// Completed with multiple iterations each having file output.
/// The Magic adapter cannot parse the file content, so `process_results` fails
/// and the job is marked Failed.
#[tokio::test]
async fn channel_completed_multiple_iterations_with_file_output() {
    let server = TestServer::new().await;
    let (mut ws, _runner_uuid, _runner_token, job_uuid) =
        setup_claimed_job(&server, "multi-iter-file").await;

    // Send Running
    send_msg(&mut ws, &RunnerMessage::Running).await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Running);

    // Send Completed with 2 iterations, each having file output
    let mut output1 = std::collections::BTreeMap::new();
    output1.insert(
        camino::Utf8PathBuf::from("/output/iter1.txt"),
        "not parseable benchmark data".to_owned(),
    );
    let mut output2 = std::collections::BTreeMap::new();
    output2.insert(
        camino::Utf8PathBuf::from("/output/iter2.txt"),
        "also not parseable".to_owned(),
    );
    send_msg(
        &mut ws,
        &RunnerMessage::Completed {
            job: job_uuid,
            results: vec![
                JsonIterationOutput {
                    exit_code: 0,
                    stdout: None,
                    stderr: None,
                    output: Some(output1),
                },
                JsonIterationOutput {
                    exit_code: 0,
                    stdout: None,
                    stderr: None,
                    output: Some(output2),
                },
            ],
        },
    )
    .await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));
    // Magic adapter cannot parse the file content -> process_results fails -> Failed
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Failed);

    // Connection stays open. Close from client side.
    ws.close(None).await.expect("Failed to close WebSocket");
}

/// Failed with multiple iterations (partial results before failure).
/// The job should transition to Failed.
#[tokio::test]
async fn channel_failed_multiple_iterations() {
    let server = TestServer::new().await;
    let (mut ws, _runner_uuid, _runner_token, job_uuid) =
        setup_claimed_job(&server, "multi-iter-fail").await;

    // Send Running
    send_msg(&mut ws, &RunnerMessage::Running).await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Running);

    // Send Failed with 2 iterations (partial results before failure)
    send_msg(
        &mut ws,
        &RunnerMessage::Failed {
            job: job_uuid,
            results: vec![
                JsonIterationOutput {
                    exit_code: 0,
                    stdout: None,
                    stderr: None,
                    output: None,
                },
                JsonIterationOutput {
                    exit_code: 1,
                    stdout: None,
                    stderr: Some("benchmark crashed on iteration 2\n".into()),
                    output: None,
                },
            ],
            error: "iteration 2 failed".to_owned(),
        },
    )
    .await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Failed);

    // Connection stays open. Close from client side.
    ws.close(None).await.expect("Failed to close WebSocket");
}

// =============================================================================
// Job Timeout Test
// =============================================================================

/// Verify that a job exceeding its configured timeout + grace period is marked Canceled.
/// This is distinct from heartbeat timeout (which marks jobs as Failed).
/// Uses tokio time manipulation to avoid waiting real wall-clock time.
#[tokio::test]
#[expect(clippy::panic)]
async fn channel_job_timeout() {
    use std::sync::{
        Arc,
        atomic::{AtomicI64, Ordering},
    };

    // Use a mock clock so that `context.clock.now()` returns controllable time.
    // Without this, `DateTime::now()` returns real wall-clock time which doesn't
    // advance with `tokio::time::advance()`.
    let base_time = bencher_json::DateTime::now().timestamp();
    let mock_time = Arc::new(AtomicI64::new(base_time));
    let time_ref = mock_time.clone();
    let clock = bencher_json::Clock::Custom(Arc::new(move || {
        bencher_json::DateTime::try_from(time_ref.load(Ordering::Relaxed)).unwrap()
    }));

    let server = TestServer::new_with_clock(3600, 1024 * 1024, clock).await;
    let admin = server.signup("Admin", "ws-jobtimeout@example.com").await;
    let org = server.create_org(&admin, "Ws jobtimeout").await;
    let project = server
        .create_project(&admin, &org, "Ws jobtimeout proj")
        .await;

    let runner = create_runner(&server, &admin.token, "Runner jobtimeout").await;
    let runner_token = runner.token.to_string();

    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let (_, spec_id) = insert_test_spec(&server);

    // Insert a job with a short timeout (10 seconds)
    let job_uuid = insert_test_job_with_timeout(&server, report_id, spec_id, 10);

    let runner_id = get_runner_id(&server, runner.uuid);
    associate_runner_spec(&server, runner_id, spec_id);

    // Connect channel, send Ready, receive Job
    let mut ws = connect_channel(&server, runner.uuid, &runner_token).await;
    let ready = RunnerMessage::Ready {
        poll_timeout: Some(PollTimeout::try_from(5).expect("Invalid poll timeout")),
    };
    send_msg(&mut ws, &ready).await;
    let response = recv_msg(&mut ws).await;
    match response {
        ServerMessage::Job(job) => assert_eq!(job.uuid, job_uuid),
        ServerMessage::Ack | ServerMessage::NoJob | ServerMessage::Cancel => {
            panic!("Expected Job message, got: {response:?}");
        },
    }

    // Send Running to start the job (sets the `started` timestamp via mock clock)
    send_msg(&mut ws, &RunnerMessage::Running).await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Running);

    // Send Heartbeat to confirm job is active
    send_msg(&mut ws, &RunnerMessage::Heartbeat).await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));

    // Advance mock clock past job timeout (10s) + grace period (60s) = 70s.
    mock_time.fetch_add(75, Ordering::Relaxed);

    // Pause tokio time and advance past the heartbeat timeout (5s) so the
    // handler fires. It will read the mock clock which is now 75s ahead,
    // see the job has exceeded its timeout + grace period, and mark it Canceled.
    tokio::time::pause();
    tokio::time::advance(std::time::Duration::from_secs(75)).await;
    tokio::time::resume();

    // The server's timeout handler should have fired and the WS stream ends.
    match ws.next().await {
        None | Some(Ok(Message::Close(_)) | Err(_)) => {
            // Connection closed as expected
        },
        Some(Ok(other)) => {
            panic!("Expected connection to close from job timeout, got: {other:?}");
        },
    }

    // Verify job is marked as Canceled (not Failed — distinguishes from heartbeat timeout)
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Canceled);
}

// =============================================================================
// Token Rotation Tests
// =============================================================================

/// After rotating a runner's token, the old token cannot open a channel,
/// but the new token can.
#[tokio::test]
async fn channel_token_rotation_invalidates_old_token() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "ws-tokenrot@example.com").await;
    let org = server.create_org(&admin, "Ws tokenrot").await;
    let project = server
        .create_project(&admin, &org, "Ws tokenrot proj")
        .await;

    let runner = create_runner(&server, &admin.token, "Runner tokenrot").await;
    let original_token = runner.token.to_string();

    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let (_, spec_id) = insert_test_spec(&server);
    let job_uuid = insert_test_job(&server, report_id, spec_id);

    let runner_id = get_runner_id(&server, runner.uuid);
    associate_runner_spec(&server, runner_id, spec_id);

    // Open channel with original token, claim job, send Running
    let mut ws = connect_channel(&server, runner.uuid, &original_token).await;
    let ready = RunnerMessage::Ready {
        poll_timeout: Some(PollTimeout::try_from(5).expect("Invalid poll timeout")),
    };
    send_msg(&mut ws, &ready).await;
    let response = recv_msg(&mut ws).await;
    assert!(matches!(response, ServerMessage::Job(_)));

    send_msg(&mut ws, &RunnerMessage::Running).await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Running);

    // Close the WebSocket
    ws.close(None).await.expect("Failed to close WebSocket");

    // Rotate the runner token via admin API
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/token", runner.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .send()
        .await
        .expect("Rotation request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let new_runner: JsonRunnerToken = resp
        .json()
        .await
        .expect("Failed to parse rotation response");
    let new_token: String = new_runner.token.as_ref().to_owned();

    // Old token should be rejected on channel
    let url = ws_url(&server, &format!("/v0/runners/{}/channel", runner.uuid));
    let mut request = url.into_client_request().expect("Failed to build request");
    request.headers_mut().insert(
        bencher_json::AUTHORIZATION,
        bencher_json::bearer_header(&original_token)
            .parse()
            .expect("Invalid header"),
    );
    match tokio_tungstenite::connect_async(request).await {
        Err(_) => {}, // Rejected at HTTP level
        Ok((mut ws, _)) => {
            assert_ws_closed(&mut ws).await;
        },
    }

    // New token should work for channel connection
    let mut ws = connect_channel(&server, runner.uuid, &new_token).await;
    // The job is Running; we can send a Heartbeat via the channel to verify auth works
    // But first we need to send Ready (the channel starts in Idle state).
    // Since the job is already Running, sending Ready will poll for a new Pending job.
    // There are no more pending jobs, so we'll get NoJob. That's fine — it proves auth works.
    let ready = RunnerMessage::Ready {
        poll_timeout: Some(PollTimeout::try_from(1).expect("Invalid poll timeout")),
    };
    send_msg(&mut ws, &ready).await;
    let resp = recv_msg(&mut ws).await;
    assert!(
        matches!(resp, ServerMessage::NoJob),
        "Expected NoJob (job is already Running), got: {resp:?}"
    );

    ws.close(None).await.expect("Failed to close WebSocket");
}

// =============================================================================
// Bug Fix 1: handle_running returns Cancel on concurrent cancellation
// =============================================================================

/// When a job is canceled between channel connect and the Running message,
/// the server should return Cancel (not Ack) so the runner doesn't execute.
#[tokio::test]
async fn channel_running_cancel_on_concurrent_cancellation() {
    let server = TestServer::new().await;
    let (mut ws, _runner_uuid, _runner_token, job_uuid) =
        setup_claimed_job(&server, "run-cancel").await;

    // Cancel the job in DB (simulating concurrent user cancellation)
    set_job_status(&server, job_uuid, JobStatus::Canceled);

    // Send Running — the conditional UPDATE will match 0 rows because job is
    // now Canceled (not Claimed or Running). handle_running re-reads the job,
    // detects the cancellation, and returns Cancel.
    send_msg(&mut ws, &RunnerMessage::Running).await;
    let resp = recv_msg(&mut ws).await;
    assert!(
        matches!(resp, ServerMessage::Cancel),
        "Expected Cancel when Running sent on concurrently-canceled job, got: {resp:?}"
    );

    // Job remains Canceled
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Canceled);

    // Connection stays open. Close from client side.
    ws.close(None).await.expect("Failed to close WebSocket");
}

// =============================================================================
// Bug Fix 2: handle_completed/handle_failed idempotency on concurrent changes
// =============================================================================

/// Completed message after job was concurrently canceled should succeed gracefully.
/// Before the fix, this would return an error and close the connection ungracefully.
#[tokio::test]
async fn channel_completed_after_concurrent_cancel() {
    let server = TestServer::new().await;
    let (mut ws, _runner_uuid, _runner_token, job_uuid) =
        setup_claimed_job(&server, "done-vs-cancel").await;

    // Transition to Running
    send_msg(&mut ws, &RunnerMessage::Running).await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Running);

    // Cancel the job in DB (simulating timeout task or admin action)
    set_job_status(&server, job_uuid, JobStatus::Canceled);

    // Send Completed — the UPDATE matches 0 rows (status is Canceled, not Running).
    // After the fix, handle_completed re-reads, sees terminal state, returns Ok.
    send_msg(
        &mut ws,
        &RunnerMessage::Completed {
            job: job_uuid,
            results: vec![JsonIterationOutput {
                exit_code: 0,
                stdout: Some("benchmark results\n".into()),
                stderr: None,
                output: None,
            }],
        },
    )
    .await;
    let resp = recv_msg(&mut ws).await;
    assert!(
        matches!(resp, ServerMessage::Ack),
        "Expected Ack for Completed after concurrent cancel, got: {resp:?}"
    );

    // Job stays Canceled (not overwritten to Completed)
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Canceled);

    // Connection stays open. Close from client side.
    ws.close(None).await.expect("Failed to close WebSocket");
}

/// Completed message after job was concurrently failed (by timeout) should succeed.
#[tokio::test]
async fn channel_completed_after_concurrent_failure() {
    let server = TestServer::new().await;
    let (mut ws, _runner_uuid, _runner_token, job_uuid) =
        setup_claimed_job(&server, "done-vs-fail").await;

    // Transition to Running
    send_msg(&mut ws, &RunnerMessage::Running).await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Running);

    // Mark job Failed in DB (simulating heartbeat timeout on a different connection)
    set_job_status(&server, job_uuid, JobStatus::Failed);

    // Send Completed — should gracefully handle the race
    send_msg(
        &mut ws,
        &RunnerMessage::Completed {
            job: job_uuid,
            results: vec![JsonIterationOutput {
                exit_code: 0,
                stdout: None,
                stderr: None,
                output: None,
            }],
        },
    )
    .await;
    let resp = recv_msg(&mut ws).await;
    assert!(
        matches!(resp, ServerMessage::Ack),
        "Expected Ack for Completed after concurrent failure, got: {resp:?}"
    );

    // Job stays Failed (the concurrent timeout won the race)
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Failed);

    // Connection stays open. Close from client side.
    ws.close(None).await.expect("Failed to close WebSocket");
}

/// Completed message when job is already Completed (idempotent duplicate).
#[tokio::test]
async fn channel_completed_idempotent_duplicate() {
    let server = TestServer::new().await;
    let (mut ws, _runner_uuid, _runner_token, job_uuid) =
        setup_claimed_job(&server, "done-idem").await;

    // Transition to Running
    send_msg(&mut ws, &RunnerMessage::Running).await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));

    // Set job to Completed directly in DB (simulating a concurrent completion)
    set_job_status(&server, job_uuid, JobStatus::Completed);

    // Send Completed — this is an idempotent duplicate
    send_msg(
        &mut ws,
        &RunnerMessage::Completed {
            job: job_uuid,
            results: vec![JsonIterationOutput {
                exit_code: 0,
                stdout: None,
                stderr: None,
                output: None,
            }],
        },
    )
    .await;
    let resp = recv_msg(&mut ws).await;
    assert!(
        matches!(resp, ServerMessage::Ack),
        "Expected Ack for idempotent Completed, got: {resp:?}"
    );

    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Completed);

    // Connection stays open. Close from client side.
    ws.close(None).await.expect("Failed to close WebSocket");
}

/// Failed message after job was concurrently canceled should succeed gracefully.
#[tokio::test]
async fn channel_failed_after_concurrent_cancel() {
    let server = TestServer::new().await;
    let (mut ws, _runner_uuid, _runner_token, job_uuid) =
        setup_claimed_job(&server, "fail-vs-cancel").await;

    // Transition to Running
    send_msg(&mut ws, &RunnerMessage::Running).await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Running);

    // Cancel the job in DB
    set_job_status(&server, job_uuid, JobStatus::Canceled);

    // Send Failed — should handle the race gracefully
    send_msg(
        &mut ws,
        &RunnerMessage::Failed {
            job: job_uuid,
            results: vec![JsonIterationOutput {
                exit_code: 1,
                stdout: None,
                stderr: None,
                output: None,
            }],
            error: "benchmark crashed".to_owned(),
        },
    )
    .await;
    let resp = recv_msg(&mut ws).await;
    assert!(
        matches!(resp, ServerMessage::Ack),
        "Expected Ack for Failed after concurrent cancel, got: {resp:?}"
    );

    // Job stays Canceled
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Canceled);

    // Connection stays open. Close from client side.
    ws.close(None).await.expect("Failed to close WebSocket");
}

/// Failed message after job was concurrently completed should succeed gracefully.
#[tokio::test]
async fn channel_failed_after_concurrent_completion() {
    let server = TestServer::new().await;
    let (mut ws, _runner_uuid, _runner_token, job_uuid) =
        setup_claimed_job(&server, "fail-vs-done").await;

    // Transition to Running
    send_msg(&mut ws, &RunnerMessage::Running).await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Running);

    // Set job to Completed in DB (simulating a race)
    set_job_status(&server, job_uuid, JobStatus::Completed);

    // Send Failed — should handle gracefully
    send_msg(
        &mut ws,
        &RunnerMessage::Failed {
            job: job_uuid,
            results: vec![JsonIterationOutput {
                exit_code: 137,
                stdout: None,
                stderr: None,
                output: None,
            }],
            error: "killed".to_owned(),
        },
    )
    .await;
    let resp = recv_msg(&mut ws).await;
    assert!(
        matches!(resp, ServerMessage::Ack),
        "Expected Ack for Failed after concurrent completion, got: {resp:?}"
    );

    // Job stays Completed (the other path won)
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Completed);

    // Connection stays open. Close from client side.
    ws.close(None).await.expect("Failed to close WebSocket");
}

/// Failed message when job is already Failed (idempotent duplicate).
#[tokio::test]
async fn channel_failed_idempotent_duplicate() {
    let server = TestServer::new().await;
    let (mut ws, _runner_uuid, _runner_token, job_uuid) =
        setup_claimed_job(&server, "fail-idem").await;

    // Transition to Running
    send_msg(&mut ws, &RunnerMessage::Running).await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));

    // Set job to Failed directly in DB
    set_job_status(&server, job_uuid, JobStatus::Failed);

    // Send Failed — idempotent duplicate
    send_msg(
        &mut ws,
        &RunnerMessage::Failed {
            job: job_uuid,
            results: vec![JsonIterationOutput {
                exit_code: 1,
                stdout: None,
                stderr: None,
                output: None,
            }],
            error: "crash".to_owned(),
        },
    )
    .await;
    let resp = recv_msg(&mut ws).await;
    assert!(
        matches!(resp, ServerMessage::Ack),
        "Expected Ack for idempotent Failed, got: {resp:?}"
    );

    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Failed);

    // Connection stays open. Close from client side.
    ws.close(None).await.expect("Failed to close WebSocket");
}

/// Completed still errors on unexpected non-terminal state (Claimed).
/// This verifies the fix didn't weaken the safety check.
#[tokio::test]
async fn channel_completed_rejects_non_terminal_unexpected_state() {
    let server = TestServer::new().await;
    let (mut ws, _runner_uuid, _runner_token, job_uuid) =
        setup_claimed_job(&server, "done-bad-state").await;

    // Job is Claimed (not Running) — send Completed directly
    send_msg(
        &mut ws,
        &RunnerMessage::Completed {
            job: job_uuid,
            results: vec![JsonIterationOutput {
                exit_code: 0,
                stdout: None,
                stderr: None,
                output: None,
            }],
        },
    )
    .await;

    // Should still error — Claimed is not a terminal state, so the error branch fires
    assert_ws_closed(&mut ws).await;

    // Job should remain Claimed
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Claimed);
}

// =============================================================================
// Bug Fix 3: Heartbeat handler checks job timeout
// =============================================================================

/// Heartbeat detects job timeout and returns Cancel, even while runner is active.
/// This is the key fix: before, only `handle_timeout` (triggered by WS silence)
/// checked job timeout. Now `handle_heartbeat` also checks, so an active runner
/// sending heartbeats can't hold a job past its timeout.
#[tokio::test]
async fn channel_heartbeat_detects_job_timeout() {
    use std::sync::{
        Arc,
        atomic::{AtomicI64, Ordering},
    };

    let base_time = bencher_json::DateTime::now().timestamp();
    let mock_time = Arc::new(AtomicI64::new(base_time));
    let time_ref = mock_time.clone();
    let clock = bencher_json::Clock::Custom(Arc::new(move || {
        bencher_json::DateTime::try_from(time_ref.load(Ordering::Relaxed)).unwrap()
    }));

    let server = TestServer::new_with_clock(3600, 1024 * 1024, clock).await;
    let admin = server.signup("Admin", "ws-hb-timeout@example.com").await;
    let org = server.create_org(&admin, "Ws hb timeout").await;
    let project = server
        .create_project(&admin, &org, "Ws hb timeout proj")
        .await;

    let runner = create_runner(&server, &admin.token, "Runner hb timeout").await;
    let runner_token = runner.token.to_string();

    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let (_, spec_id) = insert_test_spec(&server);

    // Job with 10 second timeout
    let job_uuid = insert_test_job_with_timeout(&server, report_id, spec_id, 10);

    let runner_id = get_runner_id(&server, runner.uuid);
    associate_runner_spec(&server, runner_id, spec_id);

    // Connect channel, send Ready, receive Job
    let mut ws = connect_channel(&server, runner.uuid, &runner_token).await;
    let ready = RunnerMessage::Ready {
        poll_timeout: Some(PollTimeout::try_from(5).expect("Invalid poll timeout")),
    };
    send_msg(&mut ws, &ready).await;
    let response = recv_msg(&mut ws).await;
    assert!(matches!(response, ServerMessage::Job(_)));

    // Send Running (sets `started` timestamp via mock clock)
    send_msg(&mut ws, &RunnerMessage::Running).await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Running);

    // Heartbeat while within timeout should return Ack
    mock_time.fetch_add(5, Ordering::Relaxed);
    send_msg(&mut ws, &RunnerMessage::Heartbeat).await;
    let resp = recv_msg(&mut ws).await;
    assert!(
        matches!(resp, ServerMessage::Ack),
        "Expected Ack for heartbeat within timeout, got: {resp:?}"
    );
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Running);

    // Advance mock clock past timeout (10s) + grace period (60s) = 70s
    // Total elapsed is now 5 + 70 = 75s
    mock_time.fetch_add(70, Ordering::Relaxed);

    // Send Heartbeat — should detect timeout and return Cancel
    send_msg(&mut ws, &RunnerMessage::Heartbeat).await;
    let resp = recv_msg(&mut ws).await;
    assert!(
        matches!(resp, ServerMessage::Cancel),
        "Expected Cancel when heartbeat detects job timeout, got: {resp:?}"
    );

    // Job should be Canceled (timeout exceeded, not Failed)
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Canceled);

    // Connection stays open. Close from client side.
    ws.close(None).await.expect("Failed to close WebSocket");
}

/// Heartbeat does NOT cancel when job is within timeout + grace period.
/// This verifies the timeout check doesn't trigger too early.
#[tokio::test]
async fn channel_heartbeat_no_false_timeout() {
    use std::sync::{
        Arc,
        atomic::{AtomicI64, Ordering},
    };

    let base_time = bencher_json::DateTime::now().timestamp();
    let mock_time = Arc::new(AtomicI64::new(base_time));
    let time_ref = mock_time.clone();
    let clock = bencher_json::Clock::Custom(Arc::new(move || {
        bencher_json::DateTime::try_from(time_ref.load(Ordering::Relaxed)).unwrap()
    }));

    let server = TestServer::new_with_clock(3600, 1024 * 1024, clock).await;
    let admin = server.signup("Admin", "ws-hb-noto@example.com").await;
    let org = server.create_org(&admin, "Ws hb noto").await;
    let project = server.create_project(&admin, &org, "Ws hb noto proj").await;

    let runner = create_runner(&server, &admin.token, "Runner hb noto").await;
    let runner_token = runner.token.to_string();

    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let (_, spec_id) = insert_test_spec(&server);

    // Job with 10 second timeout (grace period is 60s, so limit = 70s)
    let job_uuid = insert_test_job_with_timeout(&server, report_id, spec_id, 10);

    let runner_id = get_runner_id(&server, runner.uuid);
    associate_runner_spec(&server, runner_id, spec_id);

    // Connect channel, send Ready, receive Job
    let mut ws = connect_channel(&server, runner.uuid, &runner_token).await;
    let ready = RunnerMessage::Ready {
        poll_timeout: Some(PollTimeout::try_from(5).expect("Invalid poll timeout")),
    };
    send_msg(&mut ws, &ready).await;
    let response = recv_msg(&mut ws).await;
    assert!(matches!(response, ServerMessage::Job(_)));

    // Send Running
    send_msg(&mut ws, &RunnerMessage::Running).await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));

    // Advance to just under the limit (69s, limit is 70s)
    mock_time.fetch_add(69, Ordering::Relaxed);

    // Heartbeat should still return Ack (within timeout + grace)
    send_msg(&mut ws, &RunnerMessage::Heartbeat).await;
    let resp = recv_msg(&mut ws).await;
    assert!(
        matches!(resp, ServerMessage::Ack),
        "Expected Ack when within timeout+grace, got: {resp:?}"
    );
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Running);

    ws.close(None).await.expect("Failed to close WebSocket");
}

/// Heartbeat timeout check before job has started (no `started` timestamp).
/// Jobs in Claimed state have no `started`, so the timeout check should be skipped.
#[tokio::test]
async fn channel_heartbeat_timeout_skipped_before_running() {
    use std::sync::{
        Arc,
        atomic::{AtomicI64, Ordering},
    };

    let base_time = bencher_json::DateTime::now().timestamp();
    let mock_time = Arc::new(AtomicI64::new(base_time));
    let time_ref = mock_time.clone();
    let clock = bencher_json::Clock::Custom(Arc::new(move || {
        bencher_json::DateTime::try_from(time_ref.load(Ordering::Relaxed)).unwrap()
    }));

    let server = TestServer::new_with_clock(3600, 1024 * 1024, clock).await;
    let admin = server.signup("Admin", "ws-hb-nostart@example.com").await;
    let org = server.create_org(&admin, "Ws hb nostart").await;
    let project = server
        .create_project(&admin, &org, "Ws hb nostart proj")
        .await;

    let runner = create_runner(&server, &admin.token, "Runner hb nostart").await;
    let runner_token = runner.token.to_string();

    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let (_, spec_id) = insert_test_spec(&server);

    // Job with very short timeout (1 second)
    let job_uuid = insert_test_job_with_timeout(&server, report_id, spec_id, 1);

    let runner_id = get_runner_id(&server, runner.uuid);
    associate_runner_spec(&server, runner_id, spec_id);

    // Connect channel, send Ready, receive Job
    let mut ws = connect_channel(&server, runner.uuid, &runner_token).await;
    let ready = RunnerMessage::Ready {
        poll_timeout: Some(PollTimeout::try_from(5).expect("Invalid poll timeout")),
    };
    send_msg(&mut ws, &ready).await;
    let response = recv_msg(&mut ws).await;
    assert!(matches!(response, ServerMessage::Job(_)));

    // Advance clock well past the timeout, but don't send Running (no `started` timestamp)
    mock_time.fetch_add(500, Ordering::Relaxed);

    // Heartbeat should still return Ack because job has no `started` timestamp
    send_msg(&mut ws, &RunnerMessage::Heartbeat).await;
    let resp = recv_msg(&mut ws).await;
    assert!(
        matches!(resp, ServerMessage::Ack),
        "Expected Ack when job has no started timestamp, got: {resp:?}"
    );

    // Job should still be Claimed
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Claimed);

    ws.close(None).await.expect("Failed to close WebSocket");
}

// =============================================================================
// Multi-Job Cycle Test
// =============================================================================

/// Complete one job, then immediately send Ready and complete another job
/// on the SAME persistent channel connection.
#[tokio::test]
#[expect(clippy::panic)]
async fn channel_multi_job_cycle() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "ws-multijob@example.com").await;
    let org = server.create_org(&admin, "Ws multijob").await;
    let project = server
        .create_project(&admin, &org, "Ws multijob proj")
        .await;

    let runner = create_runner(&server, &admin.token, "Runner multijob").await;
    let runner_token = runner.token.to_string();

    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let (_, spec_id) = insert_test_spec(&server);

    // Insert two jobs
    let job_uuid_1 = insert_test_job(&server, report_id, spec_id);
    let job_uuid_2 = insert_test_job(&server, report_id, spec_id);

    let runner_id = get_runner_id(&server, runner.uuid);
    associate_runner_spec(&server, runner_id, spec_id);

    // Connect channel
    let mut ws = connect_channel(&server, runner.uuid, &runner_token).await;

    // --- Job 1 ---
    // Send Ready, receive Job
    let ready = RunnerMessage::Ready {
        poll_timeout: Some(PollTimeout::try_from(5).expect("Invalid poll timeout")),
    };
    send_msg(&mut ws, &ready).await;
    let response = recv_msg(&mut ws).await;
    let first_job_uuid = match response {
        ServerMessage::Job(job) => job.uuid,
        ServerMessage::Ack | ServerMessage::NoJob | ServerMessage::Cancel => {
            panic!("Expected Job message for first job, got: {response:?}");
        },
    };
    // Verify we got one of the two jobs
    assert!(
        first_job_uuid == job_uuid_1 || first_job_uuid == job_uuid_2,
        "Expected one of the inserted jobs"
    );

    // Send Running
    send_msg(&mut ws, &RunnerMessage::Running).await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));
    assert_eq!(get_job_status(&server, first_job_uuid), JobStatus::Running);

    // Send Completed
    send_msg(
        &mut ws,
        &RunnerMessage::Completed {
            job: first_job_uuid,
            results: vec![JsonIterationOutput {
                exit_code: 0,
                stdout: None,
                stderr: None,
                output: None,
            }],
        },
    )
    .await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));
    assert_eq!(
        get_job_status(&server, first_job_uuid),
        JobStatus::Processed
    );

    // --- Job 2 ---
    // Connection stays open. Send Ready again on the SAME connection.
    send_msg(&mut ws, &ready).await;
    let response = recv_msg(&mut ws).await;
    let second_job_uuid = match response {
        ServerMessage::Job(job) => job.uuid,
        ServerMessage::Ack | ServerMessage::NoJob | ServerMessage::Cancel => {
            panic!("Expected Job message for second job, got: {response:?}");
        },
    };
    // Should be the other job
    assert_ne!(
        first_job_uuid, second_job_uuid,
        "Second job should be different from first"
    );
    assert!(
        second_job_uuid == job_uuid_1 || second_job_uuid == job_uuid_2,
        "Expected the other inserted job"
    );

    // Send Running
    send_msg(&mut ws, &RunnerMessage::Running).await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));
    assert_eq!(get_job_status(&server, second_job_uuid), JobStatus::Running);

    // Send Completed
    send_msg(
        &mut ws,
        &RunnerMessage::Completed {
            job: second_job_uuid,
            results: vec![JsonIterationOutput {
                exit_code: 0,
                stdout: None,
                stderr: None,
                output: None,
            }],
        },
    )
    .await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));
    assert_eq!(
        get_job_status(&server, second_job_uuid),
        JobStatus::Processed
    );

    // Both jobs completed on the same persistent connection
    ws.close(None).await.expect("Failed to close WebSocket");
}

// =============================================================================
// Result Retry Tests — terminal messages during Idle state (reconnect retry)
// =============================================================================

/// Runner sends Completed during Idle state (simulating reconnect with pending result).
/// Server looks up job by UUID, processes results, sends Ack.
#[tokio::test]
async fn channel_completed_during_idle() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "ws-idle-complete@example.com").await;
    let org = server.create_org(&admin, "Ws idle complete").await;
    let project = server
        .create_project(&admin, &org, "Ws idle complete proj")
        .await;

    let runner = create_runner(&server, &admin.token, "Runner idle-complete").await;
    let runner_token = runner.token.to_string();

    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let (_, spec_id) = insert_test_spec(&server);
    let job_uuid = insert_test_job(&server, report_id, spec_id);

    let runner_id = get_runner_id(&server, runner.uuid);
    associate_runner_spec(&server, runner_id, spec_id);

    // Set job to Running with this runner (simulating a previously running job)
    set_job_runner_id(&server, job_uuid, runner_id);
    set_job_status(&server, job_uuid, JobStatus::Running);

    // Connect a fresh channel (simulating reconnect)
    let mut ws = connect_channel(&server, runner.uuid, &runner_token).await;

    // Send Completed during Idle state (before sending Ready)
    send_msg(
        &mut ws,
        &RunnerMessage::Completed {
            job: job_uuid,
            results: vec![JsonIterationOutput {
                exit_code: 0,
                stdout: None,
                stderr: None,
                output: None,
            }],
        },
    )
    .await;
    let resp = recv_msg(&mut ws).await;
    assert!(
        matches!(resp, ServerMessage::Ack),
        "Expected Ack for Completed during Idle, got: {resp:?}"
    );
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Processed);

    ws.close(None).await.expect("Failed to close WebSocket");
}

/// Runner sends Completed for an already-Processed job during Idle (idempotent).
/// Server should Ack without error.
#[tokio::test]
async fn channel_completed_during_idle_idempotent() {
    let server = TestServer::new().await;
    let (mut ws, runner_uuid, runner_token, job_uuid) =
        setup_claimed_job(&server, "idle-idem").await;

    // Complete the job normally
    send_msg(&mut ws, &RunnerMessage::Running).await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));

    send_msg(
        &mut ws,
        &RunnerMessage::Completed {
            job: job_uuid,
            results: vec![JsonIterationOutput {
                exit_code: 0,
                stdout: None,
                stderr: None,
                output: None,
            }],
        },
    )
    .await;
    let resp = recv_msg(&mut ws).await;
    assert!(matches!(resp, ServerMessage::Ack));
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Processed);

    // Close and reconnect
    ws.close(None).await.expect("Failed to close WebSocket");

    let mut ws2 = connect_channel(&server, runner_uuid, &runner_token).await;

    // Send Completed again during Idle (idempotent duplicate)
    send_msg(
        &mut ws2,
        &RunnerMessage::Completed {
            job: job_uuid,
            results: vec![JsonIterationOutput {
                exit_code: 0,
                stdout: None,
                stderr: None,
                output: None,
            }],
        },
    )
    .await;
    let resp = recv_msg(&mut ws2).await;
    assert!(
        matches!(resp, ServerMessage::Ack),
        "Expected Ack for idempotent Completed, got: {resp:?}"
    );
    // Job should still be Processed
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Processed);

    ws2.close(None).await.expect("Failed to close WebSocket");
}

/// Runner sends Failed during Idle state (simulating reconnect with pending failure).
/// Server looks up job by UUID, marks it Failed, sends Ack.
#[tokio::test]
async fn channel_failed_during_idle() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "ws-idle-fail@example.com").await;
    let org = server.create_org(&admin, "Ws idle fail").await;
    let project = server
        .create_project(&admin, &org, "Ws idle fail proj")
        .await;

    let runner = create_runner(&server, &admin.token, "Runner idle-fail").await;
    let runner_token = runner.token.to_string();

    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let (_, spec_id) = insert_test_spec(&server);
    let job_uuid = insert_test_job(&server, report_id, spec_id);

    let runner_id = get_runner_id(&server, runner.uuid);
    associate_runner_spec(&server, runner_id, spec_id);

    // Set job to Running with this runner
    set_job_runner_id(&server, job_uuid, runner_id);
    set_job_status(&server, job_uuid, JobStatus::Running);

    // Connect a fresh channel (simulating reconnect)
    let mut ws = connect_channel(&server, runner.uuid, &runner_token).await;

    // Send Failed during Idle state
    send_msg(
        &mut ws,
        &RunnerMessage::Failed {
            job: job_uuid,
            results: vec![JsonIterationOutput {
                exit_code: 1,
                stdout: None,
                stderr: None,
                output: None,
            }],
            error: "segfault on reconnect".to_owned(),
        },
    )
    .await;
    let resp = recv_msg(&mut ws).await;
    assert!(
        matches!(resp, ServerMessage::Ack),
        "Expected Ack for Failed during Idle, got: {resp:?}"
    );
    assert_eq!(get_job_status(&server, job_uuid), JobStatus::Failed);

    ws.close(None).await.expect("Failed to close WebSocket");
}
