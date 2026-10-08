//! The server keeps each runner's latest status, written only when it changes, and shows it to admins.

use std::sync::atomic::Ordering;

use api_runners::{RunnerMessage, ServerMessage};
use bencher_api_tests::{LogCapture, TestServer};
use bencher_json::{
    DateTime, JsonRunner, JsonRunners, PollTimeout, RunnerUuid,
    runner::{
        HealthFindingKind, HealthState, JsonHealthFinding, JsonPaused, JsonReady, JsonRunnerHealth,
        MAX_HEALTH_ITEMS, MAX_PAUSE_REASONS, MAX_REPORT_STRING_LEN, MdSyncAction, PauseReason,
        RunnerAvailability,
    },
};
use bencher_schema::schema;
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use http::StatusCode;

use super::{
    common::{
        WsStream, associate_runner_spec, base_timestamp, connect_channel_ws, create_runner,
        create_test_report, get_project_id, get_runner_id, insert_test_job_with_timeout,
        insert_test_spec, recv_server_msg as recv_msg, runner_metadata,
        send_runner_msg as send_msg,
    },
    mock_clock,
};

#[tokio::test]
async fn ready_stores_health() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "status-health@example.com").await;
    let runner = create_runner(&server, &admin.token, "Runner health").await;

    let mut ws = connect_channel_ws(&server, runner.uuid, runner.key.as_ref()).await;
    exchange(&mut ws, &ready(Some(failing("md0")))).await;

    let status = get_runner(&server, &admin.token, runner.uuid)
        .await
        .status
        .expect("a reported runner has a status");
    assert_eq!(status.availability, RunnerAvailability::Ready);
    assert_eq!(status.reasons, Vec::<PauseReason>::new());
    assert_eq!(status.since, None);
    assert_eq!(status.health, Some(failing("md0")));
}

// Kills "write on every message": an identical report, or one that differs only outside the
// state and findings, leaves `changed` where it was.
#[tokio::test]
async fn unchanged_report_keeps_changed() {
    let (clock, mock_time) = mock_clock(0);
    let server = TestServer::new_with_clock(3600, 1024 * 1024, clock).await;
    let admin = server.signup("Admin", "status-unchanged@example.com").await;
    let runner = create_runner(&server, &admin.token, "Runner unchanged").await;
    let mut ws = connect_channel_ws(&server, runner.uuid, runner.key.as_ref()).await;

    exchange(&mut ws, &ready(Some(failing("md0")))).await;
    let first = status_changed(&server, &admin.token, runner.uuid).await;

    mock_time.fetch_add(100, Ordering::Relaxed);
    exchange(&mut ws, &ready(Some(failing("md0")))).await;
    let mut detailed = failing("md0");
    detailed.arrays = vec![bencher_json::runner::JsonMdArray {
        name: "md0".to_owned(),
        array_state: "clean".to_owned(),
        degraded: 1,
        action: MdSyncAction::Idle,
    }];
    exchange(&mut ws, &ready(Some(detailed))).await;
    assert_eq!(
        status_changed(&server, &admin.token, runner.uuid).await,
        first
    );

    mock_time.fetch_add(100, Ordering::Relaxed);
    exchange(&mut ws, &ready(Some(failing("md1")))).await;
    assert_eq!(
        status_changed(&server, &admin.token, runner.uuid).await,
        at(200)
    );
}

// Kills dropping the sort or the dedup of findings: the same findings reordered or repeated keep `changed`.
#[tokio::test]
async fn reordered_findings_keep_changed() {
    let (clock, mock_time) = mock_clock(0);
    let server = TestServer::new_with_clock(3600, 1024 * 1024, clock).await;
    let admin = server.signup("Admin", "status-reordered@example.com").await;
    let runner = create_runner(&server, &admin.token, "Runner reordered").await;
    let mut ws = connect_channel_ws(&server, runner.uuid, runner.key.as_ref()).await;

    exchange(&mut ws, &ready(Some(failing_arrays(&["md1", "md0"])))).await;
    mock_time.fetch_add(100, Ordering::Relaxed);
    exchange(
        &mut ws,
        &ready(Some(failing_arrays(&["md0", "md1", "md1"]))),
    )
    .await;
    assert_eq!(
        status_changed(&server, &admin.token, runner.uuid).await,
        at(0)
    );
}

// Kills "compare whole reasons": a pause whose progress moves keeps `changed`, and a new reason kind moves it.
#[tokio::test]
async fn pause_progress_keeps_changed() {
    let (clock, mock_time) = mock_clock(0);
    let server = TestServer::new_with_clock(3600, 1024 * 1024, clock).await;
    let admin = server.signup("Admin", "status-progress@example.com").await;
    let runner = create_runner(&server, &admin.token, "Runner progress").await;
    let mut ws = connect_channel_ws(&server, runner.uuid, runner.key.as_ref()).await;

    exchange(&mut ws, &paused(vec![raid(1)])).await;
    mock_time.fetch_add(100, Ordering::Relaxed);
    exchange(&mut ws, &paused(vec![raid(2)])).await;
    assert_eq!(
        status_changed(&server, &admin.token, runner.uuid).await,
        at(0)
    );

    mock_time.fetch_add(100, Ordering::Relaxed);
    exchange(&mut ws, &paused(vec![raid(3), maintenance()])).await;
    assert_eq!(
        status_changed(&server, &admin.token, runner.uuid).await,
        at(200)
    );
}

#[tokio::test]
async fn pause_is_stored_and_ready_clears_it() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "status-pause@example.com").await;
    let runner = create_runner(&server, &admin.token, "Runner pause").await;
    let mut ws = connect_channel_ws(&server, runner.uuid, runner.key.as_ref()).await;

    exchange(&mut ws, &paused(vec![raid(1)])).await;
    let status = get_runner(&server, &admin.token, runner.uuid)
        .await
        .status
        .expect("a reported runner has a status");
    assert_eq!(status.availability, RunnerAvailability::Paused);
    assert_eq!(status.reasons, vec![raid(1)]);
    assert_eq!(status.since, Some(base_timestamp()));

    exchange(&mut ws, &ready(None)).await;
    let status = get_runner(&server, &admin.token, runner.uuid)
        .await
        .status
        .expect("a reported runner has a status");
    assert_eq!(status.availability, RunnerAvailability::Ready);
    assert_eq!(status.reasons, Vec::<PauseReason>::new());
    assert_eq!(status.since, None);
}

// Kills "absent overwrites": a runner too old to report health leaves the stored health alone.
#[tokio::test]
async fn ready_without_health_keeps_stored_health() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "status-old@example.com").await;
    let runner = create_runner(&server, &admin.token, "Runner old").await;
    let mut ws = connect_channel_ws(&server, runner.uuid, runner.key.as_ref()).await;

    exchange(&mut ws, &ready(Some(failing("md0")))).await;
    exchange(&mut ws, &paused(vec![maintenance()])).await;
    exchange(&mut ws, &ready(None)).await;

    let status = get_runner(&server, &admin.token, runner.uuid)
        .await
        .status
        .expect("a reported runner has a status");
    assert_eq!(status.availability, RunnerAvailability::Ready);
    assert_eq!(status.health, Some(failing("md0")));
}

// Kills "store what the runner sent": oversized lists are cut, and the message is still answered.
#[tokio::test]
async fn oversized_report_is_capped() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "status-capped@example.com").await;
    let runner = create_runner(&server, &admin.token, "Runner capped").await;
    let mut ws = connect_channel_ws(&server, runner.uuid, runner.key.as_ref()).await;

    let mut health = failing("md0");
    health.findings = (0..100)
        .map(|i| JsonHealthFinding {
            device: format!("nvme{i}"),
            kind: HealthFindingKind::MediaErrors,
            state: HealthState::Warning,
        })
        .collect();
    let message = RunnerMessage::Paused(JsonPaused {
        reasons: vec![maintenance(); MAX_PAUSE_REASONS + 4],
        since: base_timestamp(),
        ready: JsonReady::new_with_health(Some(poll_timeout()), None, Some(health)),
    });
    exchange(&mut ws, &message).await;

    let status = get_runner(&server, &admin.token, runner.uuid)
        .await
        .status
        .expect("a reported runner has a status");
    assert_eq!(status.reasons.len(), MAX_PAUSE_REASONS);
    assert_eq!(
        status.health.expect("health was reported").findings.len(),
        MAX_HEALTH_ITEMS
    );
}

// Kills storing pause reason strings uncapped: the status is stored before the pause handler caps them.
#[tokio::test]
async fn stored_pause_reason_strings_are_capped() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "status-long@example.com").await;
    let runner = create_runner(&server, &admin.token, "Runner long").await;
    let mut ws = connect_channel_ws(&server, runner.uuid, runner.key.as_ref()).await;

    let reason = |array: String| PauseReason::Raid {
        array,
        action: MdSyncAction::Check,
        done: None,
        total: None,
        speed: None,
    };
    exchange(
        &mut ws,
        &paused(vec![reason("m".repeat(2 * MAX_REPORT_STRING_LEN))]),
    )
    .await;

    let status = get_runner(&server, &admin.token, runner.uuid)
        .await
        .status
        .expect("a reported runner has a status");
    assert_eq!(
        status.reasons,
        vec![reason("m".repeat(MAX_REPORT_STRING_LEN))]
    );
}

// Kills dropping the admin gate on the runner endpoint, which now carries disk models and serials.
#[tokio::test]
async fn non_admin_cannot_read_runner_status() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "status-admin@example.com").await;
    let user = server.signup("User", "status-user@example.com").await;
    let runner = create_runner(&server, &admin.token, "Runner gated").await;

    let response = server
        .client
        .get(server.api_url(&format!("/v0/runners/{}", runner.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

// Kills "write `last_heartbeat` on every message" and "never write it".
#[tokio::test]
async fn last_heartbeat_is_written_at_most_every_five_minutes() {
    let (clock, mock_time) = mock_clock(0);
    let server = TestServer::new_with_clock(3600, 1024 * 1024, clock).await;
    let admin = server.signup("Admin", "status-alive@example.com").await;
    let runner = create_runner(&server, &admin.token, "Runner alive").await;
    let mut ws = connect_channel_ws(&server, runner.uuid, runner.key.as_ref()).await;

    exchange(&mut ws, &ready(None)).await;
    assert_eq!(
        last_heartbeat(&server, &admin.token, runner.uuid).await,
        at(0)
    );

    mock_time.fetch_add(299, Ordering::Relaxed);
    exchange(&mut ws, &paused(vec![maintenance()])).await;
    assert_eq!(
        last_heartbeat(&server, &admin.token, runner.uuid).await,
        at(0)
    );

    mock_time.fetch_add(1, Ordering::Relaxed);
    exchange(&mut ws, &ready(None)).await;
    assert_eq!(
        last_heartbeat(&server, &admin.token, runner.uuid).await,
        at(300)
    );
}

// Kills "only `Ready` and `Paused` write `last_heartbeat`": a runner busy with a long job stays current.
#[tokio::test]
async fn job_heartbeat_writes_last_heartbeat() {
    let (clock, mock_time) = mock_clock(0);
    let server = TestServer::new_with_clock(3600, 1024 * 1024, clock).await;
    let admin = server.signup("Admin", "status-job@example.com").await;
    let org = server.create_org(&admin, "Status job").await;
    let project = server.create_project(&admin, &org, "Status job proj").await;
    let runner = create_runner(&server, &admin.token, "Runner job").await;
    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let (_, spec_id) = insert_test_spec(&server);
    insert_test_job_with_timeout(&server, report_id, spec_id, 3600);
    associate_runner_spec(&server, get_runner_id(&server, runner.uuid), spec_id);

    let mut ws = connect_channel_ws(&server, runner.uuid, runner.key.as_ref()).await;
    send_msg(&mut ws, &ready(None)).await;
    let response = recv_msg(&mut ws).await;
    assert!(
        matches!(response, ServerMessage::Job(_)),
        "Expected Job, got: {response:?}"
    );
    exchange_ack(&mut ws, &RunnerMessage::Running).await;

    mock_time.fetch_add(600, Ordering::Relaxed);
    exchange_ack(&mut ws, &RunnerMessage::Heartbeat).await;
    assert_eq!(
        last_heartbeat(&server, &admin.token, runner.uuid).await,
        at(600)
    );
}

// Kills reading an unreadable status as a missing runner: the runner and the runner list still load,
// with one warning for each read.
#[tokio::test]
async fn unreadable_status_reads_as_none() {
    let capture = LogCapture::default();
    let server = TestServer::new_with_log(capture.logger()).await;
    let admin = server
        .signup("Admin", "status-unreadable@example.com")
        .await;
    let unreadable = create_runner(&server, &admin.token, "Runner unreadable").await;
    let readable = create_runner(&server, &admin.token, "Runner readable").await;
    for runner in [&unreadable, &readable] {
        let mut ws = connect_channel_ws(&server, runner.uuid, runner.key.as_ref()).await;
        exchange(&mut ws, &ready(Some(failing("md0")))).await;
    }
    corrupt_status(&server, unreadable.uuid);

    let runner = get_runner(&server, &admin.token, unreadable.uuid).await;
    assert!(
        runner.status.is_none(),
        "an unreadable status shows as none"
    );
    assert_eq!(status_read_failures(&capture), 1);

    let runners = get_runners(&server, &admin.token).await;
    let status = |uuid| {
        runners
            .iter()
            .find(|runner| runner.uuid == uuid)
            .map(|runner| runner.status.clone())
    };
    assert!(matches!(status(unreadable.uuid), Some(None)));
    assert_eq!(
        status(readable.uuid)
            .flatten()
            .and_then(|status| status.health),
        Some(failing("md0")),
        "the other runner keeps its status"
    );
    assert_eq!(status_read_failures(&capture), 2);
}

// Kills keeping a row this build cannot read: the runner's next report replaces it.
#[tokio::test]
async fn unreadable_status_is_replaced_by_the_next_report() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "status-replaced@example.com").await;
    let runner = create_runner(&server, &admin.token, "Runner replaced").await;
    let mut ws = connect_channel_ws(&server, runner.uuid, runner.key.as_ref()).await;
    exchange(&mut ws, &ready(Some(failing("md0")))).await;
    corrupt_status(&server, runner.uuid);

    exchange(&mut ws, &paused(vec![maintenance()])).await;
    let status = get_runner(&server, &admin.token, runner.uuid)
        .await
        .status
        .expect("the next report replaces the unreadable status");
    assert_eq!(status.availability, RunnerAvailability::Paused);
    assert_eq!(status.reasons, vec![maintenance()]);
}

/// Send `message` and expect the server to answer `NoJob` at the end of the poll.
async fn exchange(ws: &mut WsStream, message: &RunnerMessage) {
    send_msg(ws, message).await;
    let response = recv_msg(ws).await;
    assert!(
        matches!(response, ServerMessage::NoJob),
        "Expected NoJob, got: {response:?}"
    );
}

/// Send `message` and expect the server to acknowledge it.
async fn exchange_ack(ws: &mut WsStream, message: &RunnerMessage) {
    send_msg(ws, message).await;
    let response = recv_msg(ws).await;
    assert!(
        matches!(response, ServerMessage::Ack { .. }),
        "Expected Ack, got: {response:?}"
    );
}

fn ready(health: Option<JsonRunnerHealth>) -> RunnerMessage {
    RunnerMessage::Ready(JsonReady::new_with_health(
        Some(poll_timeout()),
        Some(runner_metadata()),
        health,
    ))
}

fn paused(reasons: Vec<PauseReason>) -> RunnerMessage {
    RunnerMessage::Paused(JsonPaused {
        reasons,
        since: base_timestamp(),
        ready: JsonReady::new(Some(poll_timeout()), Some(runner_metadata())),
    })
}

#[expect(clippy::expect_used, reason = "test helper")]
fn poll_timeout() -> PollTimeout {
    PollTimeout::try_from(1).expect("Invalid poll timeout")
}

fn raid(done: u64) -> PauseReason {
    PauseReason::Raid {
        array: "md0".to_owned(),
        action: MdSyncAction::Check,
        done: Some(done),
        total: Some(1024),
        speed: Some(2048),
    }
}

fn maintenance() -> PauseReason {
    PauseReason::Maintenance {
        marker: true,
        lock: false,
    }
}

fn failing(array: &str) -> JsonRunnerHealth {
    failing_arrays(&[array])
}

/// A failing report with one degraded finding per array, in the order given.
fn failing_arrays(arrays: &[&str]) -> JsonRunnerHealth {
    JsonRunnerHealth {
        state: HealthState::Failing,
        findings: arrays
            .iter()
            .map(|array| JsonHealthFinding {
                device: (*array).to_owned(),
                kind: HealthFindingKind::Degraded,
                state: HealthState::Failing,
            })
            .collect(),
        arrays: Vec::new(),
        nvme: Vec::new(),
    }
}

#[expect(clippy::expect_used, reason = "test helper")]
fn at(secs: i64) -> DateTime {
    DateTime::try_from(base_timestamp().timestamp() + secs).expect("Invalid timestamp")
}

#[expect(clippy::expect_used, reason = "test helper")]
async fn status_changed(server: &TestServer, token: &str, runner: RunnerUuid) -> DateTime {
    get_runner(server, token, runner)
        .await
        .status
        .expect("a reported runner has a status")
        .changed
}

#[expect(clippy::expect_used, reason = "test helper")]
async fn last_heartbeat(server: &TestServer, token: &str, runner: RunnerUuid) -> DateTime {
    get_runner(server, token, runner)
        .await
        .last_heartbeat
        .expect("a runner that sent a message has a heartbeat")
}

#[expect(clippy::expect_used, reason = "test helper")]
async fn get_runner(server: &TestServer, token: &str, runner: RunnerUuid) -> JsonRunner {
    let response = server
        .client
        .get(server.api_url(&format!("/v0/runners/{runner}")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "an admin reads the runner"
    );
    response.json().await.expect("Failed to parse the runner")
}

#[expect(clippy::expect_used, reason = "test helper")]
async fn get_runners(server: &TestServer, token: &str) -> Vec<JsonRunner> {
    let response = server
        .client
        .get(server.api_url("/v0/runners"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(response.status(), StatusCode::OK, "an admin lists runners");
    response
        .json::<JsonRunners>()
        .await
        .expect("Failed to parse the runners")
        .0
}

/// Store an availability this build does not know, as a newer server could.
#[expect(clippy::expect_used, reason = "test helper")]
fn corrupt_status(server: &TestServer, runner: RunnerUuid) {
    let runner_id = get_runner_id(server, runner);
    diesel::update(
        schema::runner_status::table.filter(schema::runner_status::runner_id.eq(runner_id)),
    )
    .set(schema::runner_status::availability.eq(2))
    .execute(&mut server.db_conn())
    .expect("Failed to corrupt the runner status");
}

fn status_read_failures(capture: &LogCapture) -> usize {
    capture
        .lines()
        .iter()
        .filter(|line| line.starts_with("Failed to read runner status"))
        .count()
}
