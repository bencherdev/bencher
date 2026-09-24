//! A job's callback fires once, from every write that makes the job terminal.

use std::sync::{
    Arc,
    atomic::{AtomicI64, Ordering},
};

use api_runners::{RunnerMessage, ServerMessage};
use bencher_api_tests::{TestOrg, TestServer, TestUser};
use bencher_config::spawn_job_recovery;
use bencher_json::{JobStatus, JobUuid, PollTimeout, runner::JsonIterationOutput};
use bencher_schema::{
    context::HeartbeatTasks,
    model::runner::{CallbackState, JobTimeout, QueryJobCallback, reprocess_completed_jobs},
};
use futures::StreamExt as _;
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::Message;

use super::{
    common::{
        assert_no_callback, assert_one_callback, assert_one_report_callback, associate_runner_spec,
        base_timestamp, callback_state, connect_channel_ws, create_runner, create_test_report,
        get_project_id, get_runner_id, insert_pending_callback, insert_pending_callback_with_body,
        insert_test_job_with_project, insert_test_job_with_timeout, insert_test_spec,
        recv_server_msg as recv_msg, runner_metadata, send_runner_msg as send_msg, set_job_status,
    },
    get_job_id, get_status, mock_clock, set_job_last_heartbeat, set_job_times,
    websocket::{setup_claimed_job, setup_reconnect_with_job_in},
};

fn completed(job: JobUuid, stdout: Option<&str>) -> RunnerMessage {
    RunnerMessage::Completed {
        job,
        results: vec![JsonIterationOutput {
            exit_code: 0,
            stdout: stdout.map(ToOwned::to_owned),
            stderr: None,
            output: None,
        }],
    }
}

async fn ack(ws: &mut super::common::WsStream) {
    let resp = recv_msg(ws).await;
    assert!(matches!(resp, ServerMessage::Ack { .. }), "{resp:?}");
}

/// A job claimed through a runner with a short timeout, a pending callback, and a clock the test moves.
#[expect(clippy::expect_used, reason = "test helper")]
async fn setup_timed_job(
    suffix: &str,
) -> (TestServer, super::common::WsStream, JobUuid, Arc<AtomicI64>) {
    let mock_time = Arc::new(AtomicI64::new(bencher_json::DateTime::TEST.timestamp()));
    let time_ref = Arc::clone(&mock_time);
    let clock = bencher_json::Clock::Custom(Arc::new(move || {
        bencher_json::DateTime::try_from(time_ref.load(Ordering::Relaxed))
            .unwrap_or(bencher_json::DateTime::TEST)
    }));
    let server = TestServer::new_with_clock(3600, 1024 * 1024, clock).await;
    let admin = server
        .signup("Admin", &format!("cb-{suffix}@example.com"))
        .await;
    let org = server.create_org(&admin, &format!("Cb {suffix}")).await;
    let project = server
        .create_project(&admin, &org, &format!("Cb {suffix} proj"))
        .await;
    let runner = create_runner(&server, &admin.token, &format!("Runner {suffix}")).await;
    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let (_, spec_id) = insert_test_spec(&server);
    // Deadline: started + 10 s timeout + 60 s grace period.
    let job_uuid = insert_test_job_with_timeout(&server, report_id, spec_id, 10);
    associate_runner_spec(&server, get_runner_id(&server, runner.uuid), spec_id);
    insert_pending_callback(&server, job_uuid);

    let mut ws = connect_channel_ws(&server, runner.uuid, runner.key.as_ref()).await;
    send_msg(
        &mut ws,
        &RunnerMessage::Ready {
            poll_timeout: Some(PollTimeout::try_from(5).expect("Invalid poll timeout")),
            runner: Some(runner_metadata()),
        },
    )
    .await;
    assert!(
        matches!(recv_msg(&mut ws).await, ServerMessage::Job(_)),
        "the runner claims the job"
    );
    send_msg(&mut ws, &RunnerMessage::Running).await;
    ack(&mut ws).await;
    (server, ws, job_uuid, mock_time)
}

// Site: `handle_timeout`, silence past the job's deadline.
#[tokio::test]
async fn a_heartbeat_timeout_past_the_deadline_fires() {
    let (server, mut ws, job_uuid, mock_time) = setup_timed_job("timeout").await;

    mock_time.fetch_add(75, Ordering::Relaxed);
    tokio::time::pause();
    tokio::time::advance(std::time::Duration::from_secs(75)).await;
    tokio::time::resume();
    match ws.next().await {
        None | Some(Ok(Message::Close(_)) | Err(_)) => {},
        Some(Ok(other)) => panic!("Expected the channel to close, got: {other:?}"),
    }

    assert_eq!(get_status(&server, job_uuid), JobStatus::Canceled);
    assert_one_callback(&server, job_uuid, "canceled").await;
}

// Site: `handle_heartbeat`, a heartbeat past the job's deadline.
#[tokio::test]
async fn a_heartbeat_past_the_deadline_fires() {
    let (server, mut ws, job_uuid, mock_time) = setup_timed_job("heartbeat").await;

    mock_time.fetch_add(75, Ordering::Relaxed);
    send_msg(&mut ws, &RunnerMessage::Heartbeat).await;
    assert!(matches!(recv_msg(&mut ws).await, ServerMessage::Cancel));

    assert_eq!(get_status(&server, job_uuid), JobStatus::Canceled);
    assert_one_callback(&server, job_uuid, "canceled").await;
    ws.close(None).await.expect("Failed to close WebSocket");
}

// Site: `handle_completed`, Processed. Its Completed write before it fires nothing.
#[tokio::test]
async fn a_processed_result_fires_once() {
    let server = TestServer::new().await;
    let (mut ws, _, _, job_uuid) = setup_claimed_job(&server, "cb-processed").await;
    insert_pending_callback(&server, job_uuid);
    send_msg(&mut ws, &RunnerMessage::Running).await;
    ack(&mut ws).await;

    send_msg(&mut ws, &completed(job_uuid, None)).await;
    ack(&mut ws).await;
    // A duplicate result changes nothing, so it fires nothing.
    send_msg(&mut ws, &completed(job_uuid, None)).await;
    let _duplicate = recv_msg(&mut ws).await;

    assert_eq!(get_status(&server, job_uuid), JobStatus::Processed);
    assert_one_callback(&server, job_uuid, "processed").await;
    ws.close(None).await.expect("Failed to close WebSocket");
}

// Site: `handle_completed`, Failed when the results do not process.
#[tokio::test]
async fn a_result_that_does_not_process_fires_failed() {
    let server = TestServer::new().await;
    let (mut ws, _, _, job_uuid) = setup_claimed_job(&server, "cb-unprocessed").await;
    insert_pending_callback(&server, job_uuid);
    send_msg(&mut ws, &RunnerMessage::Running).await;
    ack(&mut ws).await;

    send_msg(
        &mut ws,
        &completed(job_uuid, Some("this is not valid benchmark output")),
    )
    .await;
    ack(&mut ws).await;

    assert_eq!(get_status(&server, job_uuid), JobStatus::Failed);
    assert_one_callback(&server, job_uuid, "failed").await;
    ws.close(None).await.expect("Failed to close WebSocket");
}

// Site: `handle_failed`.
#[tokio::test]
async fn a_failed_result_fires() {
    let server = TestServer::new().await;
    let (mut ws, _, _, job_uuid) = setup_claimed_job(&server, "cb-failed").await;
    insert_pending_callback(&server, job_uuid);
    send_msg(&mut ws, &RunnerMessage::Running).await;
    ack(&mut ws).await;

    send_msg(
        &mut ws,
        &RunnerMessage::Failed {
            job: job_uuid,
            results: Vec::new(),
            error: "benchmark exited non-zero".to_owned(),
        },
    )
    .await;
    ack(&mut ws).await;

    assert_eq!(get_status(&server, job_uuid), JobStatus::Failed);
    assert_one_callback(&server, job_uuid, "failed").await;
    ws.close(None).await.expect("Failed to close WebSocket");
}

// Site: `handle_canceled`.
#[tokio::test]
async fn a_canceled_result_fires() {
    let server = TestServer::new().await;
    let (mut ws, _, _, job_uuid) = setup_claimed_job(&server, "cb-canceled").await;
    insert_pending_callback(&server, job_uuid);
    send_msg(&mut ws, &RunnerMessage::Running).await;
    ack(&mut ws).await;

    send_msg(&mut ws, &RunnerMessage::Canceled { job: job_uuid }).await;
    ack(&mut ws).await;

    assert_eq!(get_status(&server, job_uuid), JobStatus::Canceled);
    assert_one_callback(&server, job_uuid, "canceled").await;
    ws.close(None).await.expect("Failed to close WebSocket");
}

/// A report in a project of its own.
async fn create_decoy_report(server: &TestServer, admin: &TestUser, org: &TestOrg, name: &str) {
    let project = server.create_project(admin, org, name).await;
    create_test_report(server, get_project_id(server, project.slug.as_ref()));
}

/// A job claimed through a runner, with a pending callback that has no body, and the admin
/// who can read the job's report.
#[expect(clippy::expect_used, reason = "test helper")]
async fn setup_report_callback(
    suffix: &str,
) -> (TestServer, TestUser, super::common::WsStream, JobUuid) {
    let server = TestServer::new().await;
    let admin = server
        .signup("Admin", &format!("cb-report-{suffix}@example.com"))
        .await;
    let org = server
        .create_org(&admin, &format!("Cb report {suffix}"))
        .await;
    let project = server
        .create_project(&admin, &org, &format!("Cb report {suffix} proj"))
        .await;
    let runner = create_runner(&server, &admin.token, &format!("Runner {suffix}")).await;
    // Reports on either side of the job's, so only the job's own report can match.
    create_decoy_report(
        &server,
        &admin,
        &org,
        &format!("Cb report {suffix} decoy before"),
    )
    .await;
    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let (_, spec_id) = insert_test_spec(&server);
    let job_uuid = insert_test_job_with_project(&server, report_id, project.uuid, spec_id);
    create_decoy_report(
        &server,
        &admin,
        &org,
        &format!("Cb report {suffix} decoy after"),
    )
    .await;
    associate_runner_spec(&server, get_runner_id(&server, runner.uuid), spec_id);
    insert_pending_callback_with_body(&server, job_uuid, None);

    let mut ws = connect_channel_ws(&server, runner.uuid, runner.key.as_ref()).await;
    send_msg(
        &mut ws,
        &RunnerMessage::Ready {
            poll_timeout: Some(PollTimeout::try_from(5).expect("Invalid poll timeout")),
            runner: Some(runner_metadata()),
        },
    )
    .await;
    assert!(
        matches!(recv_msg(&mut ws).await, ServerMessage::Job(_)),
        "the runner claims the job"
    );
    send_msg(&mut ws, &RunnerMessage::Running).await;
    ack(&mut ws).await;
    (server, admin, ws, job_uuid)
}

// Without a body, a processed job's callback sends its report, results and all.
#[tokio::test]
async fn a_callback_without_a_body_sends_the_processed_report() {
    let (server, admin, mut ws, job_uuid) = setup_report_callback("processed").await;
    let results = serde_json::json!({ "bench_a": { "latency": { "value": 42.0 } } });

    send_msg(&mut ws, &completed(job_uuid, Some(&results.to_string()))).await;
    ack(&mut ws).await;

    assert_eq!(get_status(&server, job_uuid), JobStatus::Processed);
    let report = assert_one_report_callback(&server, &admin, job_uuid).await;
    assert_eq!(report["job"], serde_json::json!(job_uuid));
    assert_eq!(
        report["results"][0][0]["benchmark"]["name"], "bench_a",
        "the report holds the job's results"
    );
    ws.close(None).await.expect("Failed to close WebSocket");
}

// Two terminal paths on one job: the first write fires, and every later one finds the job
// already terminal and changes nothing.
#[tokio::test]
async fn two_terminal_paths_deliver_once() {
    let (server, mut ws, job_uuid, mock_time) = setup_timed_job("race").await;

    mock_time.fetch_add(75, Ordering::Relaxed);
    send_msg(&mut ws, &RunnerMessage::Heartbeat).await;
    assert!(matches!(recv_msg(&mut ws).await, ServerMessage::Cancel));
    send_msg(&mut ws, &RunnerMessage::Canceled { job: job_uuid }).await;
    ack(&mut ws).await;
    send_msg(&mut ws, &completed(job_uuid, None)).await;
    let _late = recv_msg(&mut ws).await;

    assert_eq!(get_status(&server, job_uuid), JobStatus::Canceled);
    assert_one_callback(&server, job_uuid, "canceled").await;
    ws.close(None).await.expect("Failed to close WebSocket");
}

// A late result for a job that another path already made terminal fires nothing.
#[tokio::test]
async fn a_discarded_late_result_fires_nothing() {
    let server = TestServer::new().await;
    let (mut ws, job_uuid, _) =
        setup_reconnect_with_job_in(&server, "cb-late", JobStatus::Canceled).await;
    insert_pending_callback(&server, job_uuid);

    for message in [
        completed(job_uuid, None),
        RunnerMessage::Failed {
            job: job_uuid,
            results: Vec::new(),
            error: "late".to_owned(),
        },
        RunnerMessage::Canceled { job: job_uuid },
    ] {
        send_msg(&mut ws, &message).await;
        ack(&mut ws).await;
    }

    assert_no_callback(&server, job_uuid).await;
    ws.close(None).await.expect("Failed to close WebSocket");
}

// Site: `check_job_timeout`, the heartbeat timeout task at an Unknown job's deadline.
#[tokio::test]
async fn the_deadline_of_an_unknown_job_fires() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "cb-deadline@example.com").await;
    let org = server.create_org(&admin, "Cb Deadline").await;
    let project = server
        .create_project(&admin, &org, "Cb Deadline proj")
        .await;
    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let (_, spec_id) = insert_test_spec(&server);
    let job_uuid = insert_test_job_with_timeout(&server, report_id, spec_id, 10);
    set_job_status(&server, job_uuid, JobStatus::Unknown);
    set_job_times(
        &server,
        job_uuid,
        Some(base_timestamp()),
        Some(base_timestamp()),
    );
    set_job_last_heartbeat(&server, job_uuid, base_timestamp());
    insert_pending_callback(&server, job_uuid);

    // Past the deadline of started + 10 s timeout + 60 s grace period.
    let (clock, _mock_time) = mock_clock(71);
    tokio::time::pause();
    JobTimeout {
        heartbeat: std::time::Duration::from_secs(5),
        grace_period: std::time::Duration::from_mins(1),
    }
    .spawn_heartbeat_timeout(
        slog::Logger::root(slog::Discard, slog::o!()),
        Arc::new(Mutex::new(server.db_conn())),
        get_job_id(&server, job_uuid),
        &HeartbeatTasks::new(),
        clock,
        server.context().callbacks.clone(),
    );
    tokio::task::yield_now().await;
    tokio::time::advance(std::time::Duration::from_secs(6)).await;
    tokio::task::yield_now().await;
    tokio::time::resume();

    assert_eq!(get_status(&server, job_uuid), JobStatus::Canceled);
    assert_one_callback(&server, job_uuid, "canceled").await;
}

/// A Completed job, with its output stored when `stdout` is given, and a pending callback.
#[expect(clippy::expect_used, reason = "test helper")]
async fn setup_completed_job(suffix: &str, stdout: Option<&str>) -> (TestServer, JobUuid) {
    let server = TestServer::new().await;
    let admin = server
        .signup("Admin", &format!("cb-{suffix}@example.com"))
        .await;
    let org = server.create_org(&admin, &format!("Cb {suffix}")).await;
    let project = server
        .create_project(&admin, &org, &format!("Cb {suffix} proj"))
        .await;
    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let (_, spec_id) = insert_test_spec(&server);
    let job_uuid = insert_test_job_with_project(&server, report_id, project.uuid, spec_id);
    set_job_status(&server, job_uuid, JobStatus::Completed);
    if let Some(stdout) = stdout {
        let output = bencher_json::runner::JsonJobOutput {
            results: vec![JsonIterationOutput {
                exit_code: 0,
                stdout: (!stdout.is_empty()).then(|| stdout.to_owned()),
                stderr: None,
                output: None,
            }],
            error: None,
        };
        server
            .context()
            .oci_storage()
            .job_output()
            .put(project.uuid, job_uuid, &output)
            .await
            .expect("Failed to store job output");
    }
    insert_pending_callback(&server, job_uuid);
    (server, job_uuid)
}

fn discard() -> slog::Logger {
    slog::Logger::root(slog::Discard, slog::o!())
}

// Site: `reprocess_single_completed_job`, Processed.
#[tokio::test]
async fn a_reprocessed_job_fires_processed() {
    let (server, job_uuid) = setup_completed_job("reprocessed", Some("")).await;

    reprocess_completed_jobs(&discard(), server.context()).await;

    assert_eq!(get_status(&server, job_uuid), JobStatus::Processed);
    assert_one_callback(&server, job_uuid, "processed").await;
}

// Site: `reprocess_single_completed_job`, Failed.
#[tokio::test]
async fn a_job_that_does_not_reprocess_fires_failed() {
    let (server, job_uuid) =
        setup_completed_job("unreprocessed", Some("this is not valid benchmark output")).await;

    reprocess_completed_jobs(&discard(), server.context()).await;

    assert_eq!(get_status(&server, job_uuid), JobStatus::Failed);
    assert_one_callback(&server, job_uuid, "failed").await;
}

// Completed never fires: with no stored output the job goes back to Unknown, not terminal.
#[tokio::test]
async fn a_completed_job_that_stays_unfinished_fires_nothing() {
    let (server, job_uuid) = setup_completed_job("orphaned", None).await;

    reprocess_completed_jobs(&discard(), server.context()).await;
    server.context().callbacks.fire_pending(&discard()).await;

    assert_eq!(get_status(&server, job_uuid), JobStatus::Unknown);
    assert_no_callback(&server, job_uuid).await;
}

// Startup: the reset before the server accepts connections, then production recovery. Its
// reprocess fires the job first, and its startup fire finds the claim taken, so the receiver
// gets one request.
#[tokio::test]
async fn startup_delivers_a_callback_a_crash_left_delivering_once() {
    let (server, job_uuid) = setup_completed_job("startup", Some("")).await;
    let job_id = get_job_id(&server, job_uuid);
    // The previous process claimed it and stopped before settling it.
    assert!(
        QueryJobCallback::claim(&mut server.db_conn(), job_id, base_timestamp())
            .expect("Failed to claim")
    );

    server
        .context()
        .callbacks
        .reset_delivering(&discard())
        .await;
    assert_eq!(callback_state(&server, job_uuid), CallbackState::Pending);
    spawn_job_recovery(&discard(), server.context()).await;

    assert_eq!(get_status(&server, job_uuid), JobStatus::Processed);
    assert_one_callback(&server, job_uuid, "processed").await;
}

// Startup with a runner that reconnects and resends its result while recovery runs.
#[tokio::test]
async fn startup_and_a_reconnecting_runner_deliver_once() {
    let server = TestServer::new().await;
    let (mut ws, job_uuid, _) =
        setup_reconnect_with_job_in(&server, "cb-startup-runner", JobStatus::Unknown).await;
    insert_pending_callback(&server, job_uuid);
    server
        .context()
        .callbacks
        .reset_delivering(&discard())
        .await;

    send_msg(&mut ws, &completed(job_uuid, None)).await;
    ack(&mut ws).await;
    spawn_job_recovery(&discard(), server.context()).await;

    assert_eq!(get_status(&server, job_uuid), JobStatus::Processed);
    assert_one_callback(&server, job_uuid, "processed").await;
    ws.close(None).await.expect("Failed to close WebSocket");
}

// The startup fire delivers a pending callback on a terminal job, and leaves one on a
// running job for its terminal write.
#[tokio::test]
async fn the_startup_fire_delivers_only_on_terminal_jobs() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "cb-pending@example.com").await;
    let org = server.create_org(&admin, "Cb Pending").await;
    let project = server.create_project(&admin, &org, "Cb Pending proj").await;
    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let (_, spec_id) = insert_test_spec(&server);
    let terminal = insert_test_job_with_project(&server, report_id, project.uuid, spec_id);
    set_job_status(&server, terminal, JobStatus::Failed);
    insert_pending_callback(&server, terminal);
    let running = insert_test_job_with_project(&server, report_id, project.uuid, spec_id);
    set_job_status(&server, running, JobStatus::Running);
    insert_pending_callback(&server, running);

    server
        .context()
        .callbacks
        .reset_delivering(&discard())
        .await;
    spawn_job_recovery(&discard(), server.context()).await;

    assert_one_callback(&server, terminal, "failed").await;
    assert_eq!(callback_state(&server, running), CallbackState::Pending);
}
