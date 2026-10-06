//! Server admins get mail when a runner's disk health changes or its pause runs long, once per change.

use std::sync::atomic::Ordering;

use api_runners::{RunnerMessage, ServerMessage};
use bencher_api_tests::{LogCapture, TestServer, TestUser};
use bencher_json::{
    DateTime, PollTimeout,
    runner::{
        HealthFindingKind, HealthState, JsonHealthFinding, JsonPaused, JsonReady, JsonRunnerHealth,
        MdSyncAction, PauseReason,
    },
};
use bencher_schema::schema;
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};

use super::{
    common::{
        WsStream, base_timestamp, connect_channel_ws, create_runner, recv_server_msg as recv_msg,
        runner_metadata, send_runner_msg as send_msg,
    },
    mock_clock,
};

const HOUR: i64 = 60 * 60;

// Kills "mail on every report", "mail one admin", "no mail on recovery", and "mail on a first `ok`".
#[tokio::test]
async fn health_changes_mail_each_admin_once() {
    let capture = LogCapture::default();
    let server = TestServer::new_with_log(capture.logger()).await;
    let (first, second) = two_admins(&server, "mail-changes").await;
    let runner = create_runner(&server, &first.token, "changes").await;
    let mut ws = connect_channel_ws(&server, runner.uuid, runner.key.as_ref()).await;

    exchange(&mut ws, &ready(Some(health(HealthState::Ok, &[])))).await;
    let warning = health(
        HealthState::Warning,
        &[("nvme0", HealthFindingKind::MediaErrors)],
    );
    exchange(&mut ws, &ready(Some(warning.clone()))).await;
    exchange(&mut ws, &ready(Some(warning))).await;
    exchange(&mut ws, &ready(Some(degraded("md0")))).await;
    exchange(&mut ws, &ready(Some(health(HealthState::Ok, &[])))).await;

    for admin in [&first, &second] {
        for (state, count) in [("ok", 1), ("warning", 1), ("failing", 1)] {
            assert_eq!(
                mails(
                    &capture,
                    admin,
                    &format!("Runner changes: disk health {state}")
                ),
                count,
                "{} gets one mail for the change to {state}",
                admin.email
            );
        }
    }
}

// Kills "mail only on a change of state" (a second failing disk) and "mail when a finding clears".
#[tokio::test]
async fn new_finding_while_failing_mails() {
    let capture = LogCapture::default();
    let server = TestServer::new_with_log(capture.logger()).await;
    let admin = server.signup("Admin", "mail-finding@example.com").await;
    let runner = create_runner(&server, &admin.token, "finding").await;
    let mut ws = connect_channel_ws(&server, runner.uuid, runner.key.as_ref()).await;

    exchange(&mut ws, &ready(Some(degraded("md0")))).await;
    let both = health(
        HealthState::Failing,
        &[
            ("md0", HealthFindingKind::Degraded),
            ("md1", HealthFindingKind::Degraded),
        ],
    );
    exchange(&mut ws, &ready(Some(both))).await;
    exchange(&mut ws, &ready(Some(degraded("md1")))).await;

    assert_eq!(
        mails(&capture, &admin, "Runner finding: disk health failing"),
        2,
        "the first failure and the second disk each mail once"
    );
}

// Kills "mail on unknown": an old runner sends no health, and a state this server cannot read is not news.
#[tokio::test]
async fn unknown_health_mails_nothing() {
    let capture = LogCapture::default();
    let server = TestServer::new_with_log(capture.logger()).await;
    let admin = server.signup("Admin", "mail-unknown@example.com").await;
    let runner = create_runner(&server, &admin.token, "unknown").await;
    let mut ws = connect_channel_ws(&server, runner.uuid, runner.key.as_ref()).await;

    exchange(&mut ws, &ready(None)).await;
    exchange(&mut ws, &paused(DateTime::now())).await;
    exchange(&mut ws, &ready(None)).await;
    exchange(
        &mut ws,
        &ready(Some(health(HealthState::Other("smoking".to_owned()), &[]))),
    )
    .await;

    assert_eq!(
        mails(&capture, &admin, "Runner unknown:"),
        0,
        "nothing the server can read changed"
    );
}

// The write lock, held across the status read and write, is what makes two channels for one runner mail once.
#[tokio::test]
async fn two_channels_mail_once() {
    let capture = LogCapture::default();
    let server = TestServer::new_with_log(capture.logger()).await;
    let admin = server.signup("Admin", "mail-channels@example.com").await;
    let runner = create_runner(&server, &admin.token, "channels").await;
    let mut ws1 = connect_channel_ws(&server, runner.uuid, runner.key.as_ref()).await;
    let mut ws2 = connect_channel_ws(&server, runner.uuid, runner.key.as_ref()).await;

    let failing = ready(Some(degraded("md0")));
    send_msg(&mut ws1, &failing).await;
    send_msg(&mut ws2, &failing).await;
    for ws in [&mut ws1, &mut ws2] {
        let response = recv_msg(ws).await;
        assert!(
            matches!(response, ServerMessage::NoJob),
            "Expected NoJob, got: {response:?}"
        );
    }

    assert_eq!(
        mails(&capture, &admin, "Runner channels: disk health failing"),
        1,
        "one change, one mail"
    );
}

// Kills "no mail for a long pause", "mail again when a long pause changes reason", "no mail at its end",
// "mail at the end of a short pause", and "a runner restart starts the pause over".
#[tokio::test]
async fn long_pause_mails_once_and_its_end_mails_once() {
    let capture = LogCapture::default();
    let (clock, mock_time) = mock_clock(0);
    let server = TestServer::new_with_clock_and_log(clock, capture.logger()).await;
    let admin = server.signup("Admin", "mail-pause@example.com").await;
    let runner = create_runner(&server, &admin.token, "pause").await;
    let mut ws = connect_channel_ws(&server, runner.uuid, runner.key.as_ref()).await;

    exchange(&mut ws, &paused(at(0))).await;
    mock_time.fetch_add(HOUR, Ordering::Relaxed);
    exchange(&mut ws, &ready(None)).await;

    exchange(&mut ws, &paused(at(HOUR))).await;
    mock_time.fetch_add(6 * HOUR, Ordering::Relaxed);
    exchange(&mut ws, &paused(at(7 * HOUR))).await;
    mock_time.fetch_add(6 * HOUR + 1, Ordering::Relaxed);
    exchange(&mut ws, &paused(at(7 * HOUR))).await;
    assert_eq!(
        mails(&capture, &admin, "Runner pause: paused for over 12 hours"),
        1,
        "a pause that runs past 12 hours mails once, timed from its first start"
    );
    mock_time.fetch_add(HOUR, Ordering::Relaxed);
    exchange(&mut ws, &paused(at(7 * HOUR))).await;
    let mut raid = paused(at(7 * HOUR));
    if let RunnerMessage::Paused(pause) = &mut raid {
        pause.reasons.push(PauseReason::Raid {
            array: "md0".to_owned(),
            action: MdSyncAction::Check,
            done: None,
            total: None,
            speed: None,
        });
    }
    exchange(&mut ws, &raid).await;
    exchange(&mut ws, &ready(None)).await;

    assert_eq!(
        mails(&capture, &admin, "Runner pause: paused for over 12 hours"),
        1,
        "the long pause mails once"
    );
    assert_eq!(
        mails(&capture, &admin, "Runner pause: taking Jobs again"),
        1,
        "the long pause's end mails once and the short pause's end does not"
    );
}

// Kills timing the pause from when the server first saw it rather than from when it began.
#[tokio::test]
async fn pause_seen_late_mails_at_once() {
    let capture = LogCapture::default();
    let (clock, _mock_time) = mock_clock(0);
    let server = TestServer::new_with_clock_and_log(clock, capture.logger()).await;
    let admin = server.signup("Admin", "mail-late@example.com").await;
    let runner = create_runner(&server, &admin.token, "late").await;
    let mut ws = connect_channel_ws(&server, runner.uuid, runner.key.as_ref()).await;

    exchange(&mut ws, &paused(at(-13 * HOUR))).await;

    assert_eq!(
        mails(&capture, &admin, "Runner late: paused for over 12 hours"),
        1,
        "a pause already long when first seen mails at once"
    );
}

/// The first user, who is a server admin, and a second user made one.
#[expect(clippy::expect_used, reason = "test helper")]
async fn two_admins(server: &TestServer, prefix: &str) -> (TestUser, TestUser) {
    let first = server
        .signup("First", &format!("{prefix}-first@example.com"))
        .await;
    let second = server
        .signup("Second", &format!("{prefix}-second@example.com"))
        .await;
    diesel::update(schema::user::table.filter(schema::user::uuid.eq(second.uuid)))
        .set(schema::user::admin.eq(true))
        .execute(&mut server.db_conn())
        .expect("Failed to make the second user an admin");
    (first, second)
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

/// How many mails to `admin` the server sent with this subject, after its rabbit.
fn mails(capture: &LogCapture, admin: &TestUser, subject: &str) -> usize {
    let to = format!("<{}>", admin.email);
    let subject = format!("Subject: 🐰 {subject}");
    capture
        .lines()
        .iter()
        .filter(|line| line.contains(&to) && line.contains(&subject))
        .count()
}

fn ready(health: Option<JsonRunnerHealth>) -> RunnerMessage {
    RunnerMessage::Ready(JsonReady::new_with_health(
        Some(poll_timeout()),
        Some(runner_metadata()),
        health,
    ))
}

fn paused(since: DateTime) -> RunnerMessage {
    RunnerMessage::Paused(JsonPaused {
        reasons: vec![PauseReason::Maintenance {
            marker: true,
            lock: false,
        }],
        since,
        ready: JsonReady::new(Some(poll_timeout()), Some(runner_metadata())),
    })
}

fn degraded(array: &str) -> JsonRunnerHealth {
    health(
        HealthState::Failing,
        &[(array, HealthFindingKind::Degraded)],
    )
}

fn health(state: HealthState, findings: &[(&str, HealthFindingKind)]) -> JsonRunnerHealth {
    JsonRunnerHealth {
        findings: findings
            .iter()
            .map(|(device, kind)| JsonHealthFinding {
                device: (*device).to_owned(),
                kind: kind.clone(),
                state: state.clone(),
            })
            .collect(),
        state,
        arrays: Vec::new(),
        nvme: Vec::new(),
    }
}

#[expect(clippy::expect_used, reason = "test helper")]
fn poll_timeout() -> PollTimeout {
    PollTimeout::try_from(1).expect("Invalid poll timeout")
}

#[expect(clippy::expect_used, reason = "test helper")]
fn at(secs: i64) -> DateTime {
    DateTime::try_from(base_timestamp().timestamp() + secs).expect("Invalid timestamp")
}
