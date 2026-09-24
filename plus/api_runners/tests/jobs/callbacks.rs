//! A job's callback fires once, from every write that makes the job terminal.

use std::sync::{
    Arc,
    atomic::{AtomicI64, Ordering},
};

use api_runners::{RunnerMessage, ServerMessage};
use bencher_api_tests::{TestOrg, TestServer, TestUser};
use bencher_config::spawn_job_recovery;
use bencher_json::{JobStatus, JobUuid, PlanLevel, PollTimeout, runner::JsonIterationOutput};
use bencher_schema::{
    context::HeartbeatTasks,
    model::runner::{CallbackState, JobTimeout, QueryJobCallback, reprocess_completed_jobs},
    schema,
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
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

const SUBMIT_URL: &str = "https://receiver.example/hooks/URLPATH-7c1d?key=URLQUERY-2b9e";
const SUBMIT_AUTHORIZATION: &str = "Bearer HEADERVALUE-5e3a";
const SUBMIT_MARKERS: [&str; 4] = [
    "URLPATH-7c1d",
    "URLQUERY-2b9e",
    "HEADERVALUE-5e3a",
    "BODYTEXT-8f04",
];

fn submit_body() -> serde_json::Value {
    serde_json::json!({
        "note": "BODYTEXT-8f04",
        "job": "{{ job.uuid }}",
        "status": "{{ job.status }}",
        "report": "{{ report.uuid }}",
        "project": { "uuid": "{{ project.uuid }}", "slug": "{{ project.slug }}" },
    })
}

/// A run submitted through the API with a callback, on a job only the returned runner can claim.
struct Submitted {
    server: TestServer,
    admin: TestUser,
    runner: bencher_json::JsonRunnerKey,
    job: JobUuid,
    report: bencher_json::ReportUuid,
    project: bencher_api_tests::TestProject,
}

#[expect(clippy::expect_used, reason = "test helper")]
async fn submit_with_callback(
    suffix: &str,
    licensed: bool,
    body: Option<serde_json::Value>,
) -> Submitted {
    let server = TestServer::new().await;
    let admin = server
        .signup("Admin", &format!("submit-{suffix}@example.com"))
        .await;
    let org = server.create_org(&admin, &format!("Submit {suffix}")).await;
    if licensed {
        server
            .license_org(&admin, &org, PlanLevel::Enterprise)
            .await;
    }
    let project = server
        .create_project(&admin, &org, &format!("Submit {suffix} proj"))
        .await;
    let runner = create_runner(&server, &admin.token, &format!("Runner {suffix}")).await;
    let (spec_uuid, spec_id) = insert_test_spec(&server);
    associate_runner_spec(&server, get_runner_id(&server, runner.uuid), spec_id);

    let project_slug: &str = project.slug.as_ref();
    let mut callback = serde_json::json!({
        "url": SUBMIT_URL,
        "headers": { "Authorization": SUBMIT_AUTHORIZATION },
    });
    if let Some(body) = body
        && let Some(callback) = callback.as_object_mut()
    {
        callback.insert("body".to_owned(), body);
    }
    let body = serde_json::json!({
        "project": project_slug,
        "branch": "main",
        "testbed": "localhost",
        "start_time": "2024-01-01T00:00:00Z",
        "end_time": "2024-01-01T00:01:00Z",
        "results": [],
        "job": {
            "image": format!("localhost/{project_slug}@sha256:{}", "a".repeat(64)),
            "spec": spec_uuid,
            "callback": callback,
        },
    });
    let resp = server
        .client
        .post(server.api_url("/v0/run"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), http::StatusCode::CREATED, "submit the run");
    let report: bencher_json::JsonReport = resp.json().await.expect("Failed to parse the report");
    let job = schema::job::table
        .inner_join(schema::report::table)
        .filter(schema::report::uuid.eq(report.uuid))
        .select(schema::job::uuid)
        .first(&mut server.db_conn())
        .expect("Failed to get the run's job");
    Submitted {
        server,
        admin,
        runner,
        job,
        report: report.uuid,
        project,
    }
}

/// The runner claims the submitted job, which never carries the callback, and runs it to Processed.
#[expect(clippy::expect_used, clippy::panic, reason = "test helper")]
async fn process_submitted(submitted: &Submitted) {
    let Submitted {
        server,
        runner,
        job,
        ..
    } = submitted;
    let mut ws = connect_channel_ws(server, runner.uuid, runner.key.as_ref()).await;
    send_msg(
        &mut ws,
        &RunnerMessage::Ready {
            poll_timeout: Some(PollTimeout::try_from(5).expect("Invalid poll timeout")),
            runner: Some(runner_metadata()),
        },
    )
    .await;
    let claimed = match ws.next().await {
        Some(Ok(Message::Text(text))) => text.to_string(),
        other => panic!("Expected the claimed job, got: {other:?}"),
    };
    for marker in SUBMIT_MARKERS {
        assert!(!claimed.contains(marker), "the runner's job holds {marker}");
    }
    assert!(
        matches!(
            serde_json::from_str(&claimed).expect("Failed to parse the claimed job"),
            ServerMessage::Job(claimed) if claimed.uuid == *job
        ),
        "the runner claims the submitted job"
    );
    send_msg(&mut ws, &RunnerMessage::Running).await;
    ack(&mut ws).await;
    send_msg(&mut ws, &completed(*job, None)).await;
    ack(&mut ws).await;
    assert_eq!(
        get_status(server, *job),
        JobStatus::Processed,
        "the job is processed"
    );
    ws.close(None).await.expect("Failed to close WebSocket");
}

// A run submitted with a callback for a paid organization delivers it, rendered, when its job is processed.
#[tokio::test]
async fn a_submitted_callback_is_delivered_when_the_job_is_processed() {
    let submitted = submit_with_callback("paid", true, Some(submit_body())).await;
    assert_eq!(
        callback_state(&submitted.server, submitted.job),
        CallbackState::Pending
    );

    process_submitted(&submitted).await;

    let Submitted {
        server,
        job,
        report,
        project,
        ..
    } = &submitted;
    server.drain_callbacks().await;
    let requests = server.callback_requests();
    assert_eq!(requests.len(), 1, "one callback");
    let request = requests.first().expect("one callback");
    assert_eq!(request.url.as_str(), SUBMIT_URL, "the callback URL");
    assert_eq!(
        request
            .headers
            .get("authorization")
            .and_then(|value| value.to_str().ok()),
        Some(SUBMIT_AUTHORIZATION),
        "the customer's header"
    );
    assert_eq!(
        request
            .headers
            .get("content-type")
            .and_then(|value| value.to_str().ok()),
        Some("application/json"),
        "the default content type"
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&request.body).expect("The body is JSON"),
        serde_json::json!({
            "note": "BODYTEXT-8f04",
            "job": job,
            "status": "processed",
            "report": report,
            "project": { "uuid": project.uuid, "slug": project.slug },
        }),
        "the rendered body"
    );
    assert_eq!(callback_state(server, *job), CallbackState::Delivered);
}

// A run submitted with a callback and no paid plan is accepted, and nothing is sent when its job is processed.
#[tokio::test]
async fn a_skipped_callback_sends_nothing_when_the_job_is_processed() {
    let submitted = submit_with_callback("free", false, Some(submit_body())).await;
    assert_eq!(
        callback_state(&submitted.server, submitted.job),
        CallbackState::Skipped
    );

    process_submitted(&submitted).await;

    let Submitted { server, job, .. } = &submitted;
    server.drain_callbacks().await;
    assert_eq!(server.callback_requests().len(), 0, "no callback is sent");
    assert_eq!(callback_state(server, *job), CallbackState::Skipped);
}

// A run submitted with a callback and no body delivers the job's report, exactly as the
// report endpoint returns it.
#[tokio::test]
async fn a_submitted_callback_without_a_body_delivers_the_report() {
    let submitted = submit_with_callback("report", true, None).await;

    process_submitted(&submitted).await;

    let Submitted {
        server,
        admin,
        job,
        report,
        project,
        ..
    } = &submitted;
    server.drain_callbacks().await;
    let requests = server.callback_requests();
    assert_eq!(requests.len(), 1, "one callback");
    let request = requests.first().expect("one callback");
    assert_eq!(request.url.as_str(), SUBMIT_URL, "the callback URL");
    assert_eq!(
        request
            .headers
            .get("authorization")
            .and_then(|value| value.to_str().ok()),
        Some(SUBMIT_AUTHORIZATION),
        "the customer's header"
    );
    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project_slug}/reports/{report}")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), http::StatusCode::OK, "get the report");
    let expected: serde_json::Value = resp.json().await.expect("Failed to parse the report");
    assert_eq!(expected["job"], serde_json::json!(job));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&request.body).expect("The body is JSON"),
        expected,
        "the body is the report"
    );
    assert_eq!(callback_state(server, *job), CallbackState::Delivered);
}
