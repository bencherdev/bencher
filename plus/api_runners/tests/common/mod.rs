// Each test file (`jobs.rs`, `channel.rs`, etc.) includes this module separately,
// so not all helpers are used by every test binary.
#![allow(
    dead_code,
    unused_imports,
    reason = "shared helpers; not all used by every test binary"
)]
//! Shared test helpers for `api_runners` integration tests.
//!
//! Common helpers (`get_project_id`, `create_test_report`, `set_job_status`,
//! `base_timestamp`) are re-exported from `bencher_api_tests::helpers`.
//! Runner-specific helpers live here.

use api_runners::{RunnerMessage, ServerMessage};
pub use bencher_api_tests::helpers::{
    base_timestamp, create_test_report, get_project_id, set_job_status,
};
use bencher_api_tests::{CallbackRequest, TestServer, TestUser};
use bencher_json::{
    DateTime, JobStatus, JobUuid, JsonClaimedJob, JsonNewCallback, JsonRunnerKey, PollTimeout,
    Priority, ProjectSlug, ReportUuid, RunnerUuid, SpecUuid, runner::JsonRunnerMetadata,
};
use bencher_schema::{
    model::runner::{CallbackState, InsertJobCallback, JobId, QueryJobCallback},
    schema,
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use futures::{SinkExt as _, StreamExt as _};
use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest as _};

/// Build the standard `JsonRunnerMetadata` used in tests.
pub fn runner_metadata() -> JsonRunnerMetadata {
    JsonRunnerMetadata {
        os: bencher_json::OperatingSystem::Linux,
        arch: bencher_json::Architecture::X86_64,
        version: bencher_json::BENCHER_API_VERSION.to_owned(),
        channel: None,
        checksum: None,
    }
}

/// Create a runner via the REST API.
#[expect(clippy::expect_used, reason = "test helper")]
pub async fn create_runner(server: &TestServer, admin_token: &str, name: &str) -> JsonRunnerKey {
    let body = serde_json::json!({ "name": name });
    let resp = server
        .client
        .post(server.api_url("/v0/runners"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(admin_token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(
        resp.status(),
        http::StatusCode::CREATED,
        "Failed to create runner"
    );
    resp.json().await.expect("Failed to parse response")
}

/// Default test source IP for job insertion.
pub const TEST_SOURCE_IP: &str = "127.0.0.1";

/// Insert a test spec directly into the database. Returns the spec UUID and `spec_id`.
pub fn insert_test_spec(server: &TestServer) -> (SpecUuid, i32) {
    insert_test_spec_full(
        server,
        "linux",
        "x86_64",
        2,
        0x0001_0000_0000,
        10_737_418_240,
        false,
    )
}

/// Insert a test spec with specific values. Returns (`SpecUuid`, `spec_id`).
#[expect(clippy::expect_used, reason = "test helper")]
pub fn insert_test_spec_full(
    server: &TestServer,
    os: &str,
    architecture: &str,
    cpu: i32,
    memory: i64,
    disk: i64,
    network: bool,
) -> (SpecUuid, i32) {
    let mut conn = server.db_conn();
    let now = base_timestamp();
    let spec_uuid = SpecUuid::new();
    let name = format!("test-spec-{spec_uuid}");
    let slug = format!("test-spec-{spec_uuid}");

    diesel::insert_into(schema::spec::table)
        .values((
            schema::spec::uuid.eq(&spec_uuid),
            schema::spec::name.eq(&name),
            schema::spec::slug.eq(&slug),
            schema::spec::os.eq(os),
            schema::spec::architecture.eq(architecture),
            schema::spec::cpu.eq(cpu),
            schema::spec::memory.eq(memory),
            schema::spec::disk.eq(disk),
            schema::spec::network.eq(network),
            schema::spec::created.eq(&now),
            schema::spec::modified.eq(&now),
        ))
        .execute(&mut conn)
        .expect("Failed to insert test spec");

    let spec_id: i32 = schema::spec::table
        .filter(schema::spec::uuid.eq(&spec_uuid))
        .select(schema::spec::id)
        .first(&mut conn)
        .expect("Failed to get spec ID");

    (spec_uuid, spec_id)
}

/// Associate a spec with a runner.
#[expect(clippy::expect_used, reason = "test helper")]
pub fn associate_runner_spec(server: &TestServer, runner_id: i32, spec_id: i32) {
    let mut conn = server.db_conn();
    diesel::insert_into(schema::runner_spec::table)
        .values((
            schema::runner_spec::runner_id.eq(runner_id),
            schema::runner_spec::spec_id.eq(spec_id),
        ))
        .execute(&mut conn)
        .expect("Failed to associate runner with spec");
}

/// Insert a test job directly into the database. Returns the job UUID.
/// Looks up the `organization_id` from the project associated with the report.
pub fn insert_test_job(server: &TestServer, report_id: i32, spec_id: i32) -> JobUuid {
    let project_id = get_project_id_from_report(server, report_id);
    let organization_id = get_organization_id_from_project_id(server, project_id);
    insert_test_job_full(
        server,
        report_id,
        bencher_json::ProjectUuid::new(),
        organization_id,
        TEST_SOURCE_IP,
        Priority::Unclaimed,
        spec_id,
    )
}

/// Insert a test job with a specific project UUID. Returns the job UUID.
pub fn insert_test_job_with_project(
    server: &TestServer,
    report_id: i32,
    project_uuid: bencher_json::ProjectUuid,
    spec_id: i32,
) -> JobUuid {
    let project_id = get_project_id_from_report(server, report_id);
    let organization_id = get_organization_id_from_project_id(server, project_id);
    insert_test_job_full(
        server,
        report_id,
        project_uuid,
        organization_id,
        TEST_SOURCE_IP,
        Priority::Unclaimed,
        spec_id,
    )
}

/// Insert a test job with a custom timeout (in seconds). Returns the job UUID.
#[expect(clippy::expect_used, reason = "test helper")]
pub fn insert_test_job_with_timeout(
    server: &TestServer,
    report_id: i32,
    spec_id: i32,
    timeout_secs: i32,
) -> JobUuid {
    let mut conn = server.db_conn();
    let now = base_timestamp();
    let job_uuid = JobUuid::new();
    let project_uuid = bencher_json::ProjectUuid::new();
    let project_id = get_project_id_from_report(server, report_id);
    let organization_id = get_organization_id_from_project_id(server, project_id);

    let config = serde_json::json!({
        "registry": "https://registry.bencher.dev",
        "project": project_uuid,
        "digest": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        "timeout": timeout_secs
    });

    diesel::insert_into(schema::job::table)
        .values((
            schema::job::uuid.eq(&job_uuid),
            schema::job::report_id.eq(report_id),
            schema::job::organization_id.eq(organization_id),
            schema::job::source_ip.eq(TEST_SOURCE_IP),
            schema::job::status.eq(JobStatus::Pending),
            schema::job::spec_id.eq(spec_id),
            schema::job::config.eq(config.to_string()),
            schema::job::timeout.eq(timeout_secs),
            schema::job::priority.eq(Priority::Unclaimed),
            schema::job::created.eq(&now),
            schema::job::modified.eq(&now),
        ))
        .execute(&mut conn)
        .expect("Failed to insert test job");

    // Set spec_id on the report to match the job's spec
    diesel::update(schema::report::table.filter(schema::report::id.eq(report_id)))
        .set(schema::report::spec_id.eq(Some(spec_id)))
        .execute(&mut conn)
        .expect("Failed to set report spec_id");

    job_uuid
}

/// Insert a test job with full control over scheduling parameters.
#[expect(clippy::expect_used, reason = "test helper")]
pub fn insert_test_job_full(
    server: &TestServer,
    report_id: i32,
    project_uuid: bencher_json::ProjectUuid,
    organization_id: i32,
    source_ip: &str,
    priority: Priority,
    spec_id: i32,
) -> JobUuid {
    let mut conn = server.db_conn();
    let now = base_timestamp();
    let job_uuid = JobUuid::new();

    // Create a valid JsonJobConfig as JSON
    let config = serde_json::json!({
        "registry": "https://registry.bencher.dev",
        "project": project_uuid,
        "digest": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        "timeout": 3600
    });

    diesel::insert_into(schema::job::table)
        .values((
            schema::job::uuid.eq(&job_uuid),
            schema::job::report_id.eq(report_id),
            schema::job::organization_id.eq(organization_id),
            schema::job::source_ip.eq(source_ip),
            schema::job::status.eq(JobStatus::Pending),
            schema::job::spec_id.eq(spec_id),
            schema::job::config.eq(config.to_string()),
            schema::job::timeout.eq(3600),
            schema::job::priority.eq(priority),
            schema::job::created.eq(&now),
            schema::job::modified.eq(&now),
        ))
        .execute(&mut conn)
        .expect("Failed to insert test job");

    // Set spec_id on the report to match the job's spec
    diesel::update(schema::report::table.filter(schema::report::id.eq(report_id)))
        .set(schema::report::spec_id.eq(Some(spec_id)))
        .execute(&mut conn)
        .expect("Failed to set report spec_id");

    job_uuid
}

/// Insert a test job whose config declares a BMF version, or none. Returns the job UUID.
#[expect(clippy::expect_used, reason = "test helper")]
pub fn insert_test_job_with_bmf_version(
    server: &TestServer,
    report_id: i32,
    project_uuid: bencher_json::ProjectUuid,
    spec_id: i32,
    bmf_version: Option<u8>,
) -> JobUuid {
    let mut conn = server.db_conn();
    let now = base_timestamp();
    let job_uuid = JobUuid::new();
    let project_id = get_project_id_from_report(server, report_id);
    let organization_id = get_organization_id_from_project_id(server, project_id);

    let mut config = serde_json::json!({
        "registry": "https://registry.bencher.dev",
        "project": project_uuid,
        "digest": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        "timeout": 3600
    });
    if let Some(bmf_version) = bmf_version
        && let Some(object) = config.as_object_mut()
    {
        object.insert("bmf_version".to_owned(), serde_json::json!(bmf_version));
    }

    diesel::insert_into(schema::job::table)
        .values((
            schema::job::uuid.eq(&job_uuid),
            schema::job::report_id.eq(report_id),
            schema::job::organization_id.eq(organization_id),
            schema::job::source_ip.eq(TEST_SOURCE_IP),
            schema::job::status.eq(JobStatus::Pending),
            schema::job::spec_id.eq(spec_id),
            schema::job::config.eq(config.to_string()),
            schema::job::timeout.eq(3600),
            schema::job::priority.eq(Priority::Unclaimed),
            schema::job::created.eq(&now),
            schema::job::modified.eq(&now),
        ))
        .execute(&mut conn)
        .expect("Failed to insert test job");

    // Set spec_id on the report to match the job's spec
    diesel::update(schema::report::table.filter(schema::report::id.eq(report_id)))
        .set(schema::report::spec_id.eq(Some(spec_id)))
        .execute(&mut conn)
        .expect("Failed to set report spec_id");

    job_uuid
}

/// Insert a test job with optional fields populated. Returns the job UUID.
#[expect(clippy::expect_used, reason = "test helper")]
pub fn insert_test_job_with_optional_fields(
    server: &TestServer,
    report_id: i32,
    project_uuid: bencher_json::ProjectUuid,
    spec_id: i32,
) -> JobUuid {
    let mut conn = server.db_conn();
    let now = base_timestamp();
    let job_uuid = JobUuid::new();
    let project_id = get_project_id_from_report(server, report_id);
    let organization_id = get_organization_id_from_project_id(server, project_id);

    // Create a JsonJobConfig with optional fields populated
    let config = serde_json::json!({
        "registry": "https://registry.bencher.dev",
        "project": project_uuid,
        "digest": "sha256:abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789",
        "entrypoint": ["/bin/sh", "-c"],
        "cmd": ["cargo", "bench"],
        "env": {
            "RUST_LOG": "info",
            "CI": "true"
        },
        "timeout": 7200,
        "file_paths": ["/output/results.json", "/tmp/bench.txt"]
    });

    diesel::insert_into(schema::job::table)
        .values((
            schema::job::uuid.eq(&job_uuid),
            schema::job::report_id.eq(report_id),
            schema::job::organization_id.eq(organization_id),
            schema::job::source_ip.eq(TEST_SOURCE_IP),
            schema::job::status.eq(JobStatus::Pending),
            schema::job::spec_id.eq(spec_id),
            schema::job::config.eq(config.to_string()),
            schema::job::timeout.eq(7200),
            schema::job::priority.eq(Priority::Unclaimed),
            schema::job::created.eq(&now),
            schema::job::modified.eq(&now),
        ))
        .execute(&mut conn)
        .expect("Failed to insert test job");

    // Set spec_id on the report to match the job's spec
    diesel::update(schema::report::table.filter(schema::report::id.eq(report_id)))
        .set(schema::report::spec_id.eq(Some(spec_id)))
        .execute(&mut conn)
        .expect("Failed to set report spec_id");

    job_uuid
}

/// Insert a test job with invalid config JSON (missing required fields). Returns the job UUID.
#[expect(clippy::expect_used, reason = "test helper")]
pub fn insert_test_job_with_invalid_config(
    server: &TestServer,
    report_id: i32,
    spec_id: i32,
) -> JobUuid {
    let mut conn = server.db_conn();
    let now = base_timestamp();
    let job_uuid = JobUuid::new();
    let project_id = get_project_id_from_report(server, report_id);
    let organization_id = get_organization_id_from_project_id(server, project_id);

    // Invalid config - missing required fields like digest, timeout, etc.
    let config = serde_json::json!({
        "registry": "https://registry.bencher.dev"
    });

    diesel::insert_into(schema::job::table)
        .values((
            schema::job::uuid.eq(&job_uuid),
            schema::job::report_id.eq(report_id),
            schema::job::organization_id.eq(organization_id),
            schema::job::source_ip.eq(TEST_SOURCE_IP),
            schema::job::status.eq(JobStatus::Pending),
            schema::job::spec_id.eq(spec_id),
            schema::job::config.eq(config.to_string()),
            schema::job::timeout.eq(3600),
            schema::job::priority.eq(Priority::Unclaimed),
            schema::job::created.eq(&now),
            schema::job::modified.eq(&now),
        ))
        .execute(&mut conn)
        .expect("Failed to insert test job");

    // Set spec_id on the report to match the job's spec
    diesel::update(schema::report::table.filter(schema::report::id.eq(report_id)))
        .set(schema::report::spec_id.eq(Some(spec_id)))
        .execute(&mut conn)
        .expect("Failed to set report spec_id");

    job_uuid
}

/// Get project ID from report ID.
#[expect(clippy::expect_used, reason = "test helper")]
pub fn get_project_id_from_report(server: &TestServer, report_id: i32) -> i32 {
    let mut conn = server.db_conn();
    schema::report::table
        .filter(schema::report::id.eq(report_id))
        .select(schema::report::project_id)
        .first(&mut conn)
        .expect("Failed to get project ID from report")
}

/// Get organization ID from project ID.
pub fn get_organization_id(server: &TestServer, project_id: i32) -> i32 {
    get_organization_id_from_project_id(server, project_id)
}

/// Get organization ID from project ID (by primary key).
#[expect(clippy::expect_used, reason = "test helper")]
pub fn get_organization_id_from_project_id(server: &TestServer, project_id: i32) -> i32 {
    let mut conn = server.db_conn();
    schema::project::table
        .filter(schema::project::id.eq(project_id))
        .select(schema::project::organization_id)
        .first(&mut conn)
        .expect("Failed to get organization ID")
}

/// Set the `runner_id` directly in the database (for testing preconditions).
#[expect(clippy::expect_used, reason = "test helper")]
pub fn set_job_runner_id(server: &TestServer, job_uuid: JobUuid, runner_id: i32) {
    let mut conn = server.db_conn();
    diesel::update(schema::job::table.filter(schema::job::uuid.eq(job_uuid)))
        .set(schema::job::runner_id.eq(Some(runner_id)))
        .execute(&mut conn)
        .expect("Failed to set job runner_id");
}

/// Insert a test job with a specific created timestamp (for FIFO tiebreaker tests).
#[expect(
    clippy::too_many_arguments,
    clippy::expect_used,
    reason = "test helper"
)]
pub fn insert_test_job_with_timestamp(
    server: &TestServer,
    report_id: i32,
    project_uuid: bencher_json::ProjectUuid,
    organization_id: i32,
    source_ip: &str,
    priority: Priority,
    created: DateTime,
    spec_id: i32,
) -> JobUuid {
    let mut conn = server.db_conn();
    let job_uuid = JobUuid::new();

    let config = serde_json::json!({
        "registry": "https://registry.bencher.dev",
        "project": project_uuid,
        "digest": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        "timeout": 3600
    });

    diesel::insert_into(schema::job::table)
        .values((
            schema::job::uuid.eq(&job_uuid),
            schema::job::report_id.eq(report_id),
            schema::job::organization_id.eq(organization_id),
            schema::job::source_ip.eq(source_ip),
            schema::job::status.eq(JobStatus::Pending),
            schema::job::spec_id.eq(spec_id),
            schema::job::config.eq(config.to_string()),
            schema::job::timeout.eq(3600),
            schema::job::priority.eq(priority),
            schema::job::created.eq(&created),
            schema::job::modified.eq(&created),
        ))
        .execute(&mut conn)
        .expect("Failed to insert test job");

    // Set spec_id on the report to match the job's spec
    diesel::update(schema::report::table.filter(schema::report::id.eq(report_id)))
        .set(schema::report::spec_id.eq(Some(spec_id)))
        .execute(&mut conn)
        .expect("Failed to set report spec_id");

    job_uuid
}

/// Get the priority of a job directly from the database.
#[expect(clippy::expect_used, reason = "test helper")]
pub fn get_job_priority(server: &TestServer, job_uuid: JobUuid) -> Priority {
    let mut conn = server.db_conn();
    schema::job::table
        .filter(schema::job::uuid.eq(job_uuid))
        .select(schema::job::priority)
        .first(&mut conn)
        .expect("Failed to get job priority")
}

/// Get `runner_id` (as i32) from runner UUID.
#[expect(clippy::expect_used, reason = "test helper")]
pub fn get_runner_id(server: &TestServer, runner_uuid: RunnerUuid) -> i32 {
    let mut conn = server.db_conn();
    schema::runner::table
        .filter(schema::runner::uuid.eq(runner_uuid))
        .select(schema::runner::id)
        .first(&mut conn)
        .expect("Failed to get runner ID")
}

// =============================================================================
// WebSocket Channel Helpers
// =============================================================================

pub type WsStream =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// Convert the `TestServer` HTTP URL to a WebSocket URL.
pub fn ws_url(server: &TestServer, path: &str) -> String {
    let http_url = server.api_url(path);
    http_url.replacen("http://", "ws://", 1)
}

/// Connect to the runner channel WebSocket with authentication.
#[expect(clippy::expect_used, reason = "test helper")]
pub async fn connect_channel_ws(
    server: &TestServer,
    runner_uuid: RunnerUuid,
    runner_key: &str,
) -> WsStream {
    let url = ws_url(server, &format!("/v0/runners/{runner_uuid}/channel"));
    let mut request = url.into_client_request().expect("Failed to build request");
    request.headers_mut().insert(
        bencher_json::AUTHORIZATION,
        bencher_json::bearer_header(runner_key)
            .parse()
            .expect("Invalid header"),
    );
    let (ws_stream, _) = tokio_tungstenite::connect_async(request)
        .await
        .expect("Failed to connect WebSocket");
    ws_stream
}

/// Try to connect to the runner channel WebSocket; returns the result rather
/// than panicking so callers can assert on connection failure.
#[expect(clippy::expect_used, reason = "test helper")]
pub async fn try_connect_channel_ws(
    server: &TestServer,
    runner_uuid: RunnerUuid,
    runner_key: &str,
) -> Result<WsStream, tokio_tungstenite::tungstenite::Error> {
    let url = ws_url(server, &format!("/v0/runners/{runner_uuid}/channel"));
    let mut request = url.into_client_request().expect("Failed to build request");
    request.headers_mut().insert(
        bencher_json::AUTHORIZATION,
        bencher_json::bearer_header(runner_key)
            .parse()
            .expect("Invalid header"),
    );
    let (ws_stream, _) = tokio_tungstenite::connect_async(request).await?;
    Ok(ws_stream)
}

/// Send a `RunnerMessage` over the WebSocket.
#[expect(clippy::expect_used, reason = "test helper")]
pub async fn send_runner_msg(ws: &mut WsStream, msg: &RunnerMessage) {
    let text = serde_json::to_string(msg).expect("Failed to serialize");
    ws.send(Message::Text(text.into()))
        .await
        .expect("Failed to send message");
}

/// Receive and parse a `ServerMessage` from the WebSocket.
#[expect(
    clippy::expect_used,
    clippy::panic,
    clippy::wildcard_enum_match_arm,
    reason = "test helper"
)]
pub async fn recv_server_msg(ws: &mut WsStream) -> ServerMessage {
    let msg = ws.next().await.expect("Stream ended").expect("WS error");
    match msg {
        Message::Text(text) => serde_json::from_str(&text).expect("Failed to parse server message"),
        other => panic!("Expected text message, got: {other:?}"),
    }
}

/// Connect to the channel WS, send Ready, and return the stream and the
/// optional claimed job.
#[expect(clippy::expect_used, clippy::panic, reason = "test helper")]
pub async fn claim_via_channel(
    server: &TestServer,
    runner_uuid: RunnerUuid,
    runner_key: &str,
    poll_timeout: u32,
) -> (WsStream, Option<JsonClaimedJob>) {
    let mut ws = connect_channel_ws(server, runner_uuid, runner_key).await;
    let ready = RunnerMessage::Ready {
        poll_timeout: Some(PollTimeout::try_from(poll_timeout).expect("Invalid poll timeout")),
        runner: Some(runner_metadata()),
    };
    send_runner_msg(&mut ws, &ready).await;
    let response = recv_server_msg(&mut ws).await;
    let job = match response {
        ServerMessage::Job(job) => Some(*job),
        ServerMessage::NoJob => None,
        other @ (ServerMessage::Ack { .. }
        | ServerMessage::Cancel
        | ServerMessage::Update { .. }) => {
            panic!("Expected Job or NoJob, got: {other:?}")
        },
    };
    (ws, job)
}

/// Assert the WebSocket stream is closed (no more messages or Close frame).
#[expect(clippy::panic, reason = "test helper")]
pub async fn assert_ws_closed(ws: &mut WsStream) {
    let result = tokio::time::timeout(std::time::Duration::from_secs(1), ws.next()).await;
    match result {
        // Timed out, stream ended, close frame, or connection reset — all OK
        Err(_) | Ok(None | Some(Ok(Message::Close(_)) | Err(_))) => {},
        Ok(Some(Ok(other))) => panic!("Expected WS to be closed, got message: {other:?}"),
    }
}

// =============================================================================
// Callback Helpers
// =============================================================================

pub const CALLBACK_URL: &str = "https://receiver.example/bencher/hook";
pub const CALLBACK_AUTHORIZATION: &str = "Bearer callback-token";
/// Store a pending callback whose body has all five values, so a test sees the status each site
/// fires with, sealed with the server's key the way a submit does.
pub fn insert_pending_callback(server: &TestServer, job_uuid: JobUuid) {
    insert_pending_callback_with_body(
        server,
        job_uuid,
        Some(serde_json::json!({
            "job": { "uuid": "{{ job.uuid }}", "status": "{{ job.status }}" },
            "report": { "uuid": "{{ report.uuid }}" },
            "project": { "uuid": "{{ project.uuid }}", "slug": "{{ project.slug }}" },
        })),
    );
}

/// Store a pending callback with `body`, or with none, so it sends the report.
#[expect(clippy::expect_used, reason = "test helper")]
pub fn insert_pending_callback_with_body(
    server: &TestServer,
    job_uuid: JobUuid,
    body: Option<serde_json::Value>,
) {
    let callback = JsonNewCallback::new(
        CALLBACK_URL,
        [(
            "Authorization".to_owned(),
            CALLBACK_AUTHORIZATION.to_owned(),
        )],
        body,
    )
    .expect("Invalid callback");
    let plaintext = serde_json::to_vec(&callback).expect("Failed to serialize the callback");
    let sealed = server
        .context()
        .callback_key
        .seal(job_uuid, &plaintext)
        .expect("Failed to seal the callback");
    InsertJobCallback::pending(job_id(server, job_uuid), sealed, base_timestamp())
        .insert(&mut server.db_conn())
        .expect("Failed to insert the callback");
}

/// The stored state of a job's callback.
#[expect(clippy::expect_used, reason = "test helper")]
pub fn callback_state(server: &TestServer, job_uuid: JobUuid) -> CallbackState {
    QueryJobCallback::get(&mut server.db_conn(), job_id(server, job_uuid))
        .expect("Failed to get the callback")
        .state
}

/// Wait for every delivery, then assert the receiver got exactly one request: this job's
/// callback in `status`, with all five values rendered and the delivery headers.
#[expect(clippy::expect_used, reason = "test helper")]
pub async fn assert_one_callback(server: &TestServer, job_uuid: JobUuid, status: &str) {
    let request = one_delivered_callback(server, job_uuid).await;
    let (report_uuid, project_uuid, project_slug): (
        ReportUuid,
        bencher_json::ProjectUuid,
        ProjectSlug,
    ) = schema::job::table
        .inner_join(schema::report::table.inner_join(schema::project::table))
        .filter(schema::job::uuid.eq(job_uuid))
        .select((
            schema::report::uuid,
            schema::project::uuid,
            schema::project::slug,
        ))
        .first(&mut server.db_conn())
        .expect("Failed to get the job's report and project");
    let body: serde_json::Value =
        serde_json::from_str(&request.body).expect("The callback body is JSON");
    assert_eq!(
        body,
        serde_json::json!({
            "job": { "uuid": job_uuid, "status": status },
            "report": { "uuid": report_uuid },
            "project": { "uuid": project_uuid, "slug": project_slug },
        }),
        "the rendered body"
    );
}

/// Wait for every delivery, then assert the receiver got exactly one request: this job's
/// callback with the delivery headers and, as its body, the job's report exactly as the report
/// endpoint returns it to `user`. Returns that report.
#[expect(clippy::expect_used, reason = "test helper")]
pub async fn assert_one_report_callback(
    server: &TestServer,
    user: &TestUser,
    job_uuid: JobUuid,
) -> serde_json::Value {
    let request = one_delivered_callback(server, job_uuid).await;
    let (report_uuid, project_slug): (ReportUuid, ProjectSlug) = schema::job::table
        .inner_join(schema::report::table.inner_join(schema::project::table))
        .filter(schema::job::uuid.eq(job_uuid))
        .select((schema::report::uuid, schema::project::slug))
        .first(&mut server.db_conn())
        .expect("Failed to get the job's report and project");
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{project_slug}/reports/{report_uuid}"
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), http::StatusCode::OK, "get the report");
    let report: serde_json::Value = resp.json().await.expect("Failed to parse the report");
    let body: serde_json::Value =
        serde_json::from_str(&request.body).expect("The callback body is JSON");
    assert_eq!(body, report, "the body is the report");
    report
}

/// Wait for every delivery, then return the one request the receiver got, after asserting its
/// URL and headers and that the callback is delivered.
#[expect(clippy::expect_used, reason = "test helper")]
async fn one_delivered_callback(server: &TestServer, job_uuid: JobUuid) -> CallbackRequest {
    server.drain_callbacks().await;
    let requests = server.callback_requests();
    assert_eq!(requests.len(), 1, "one callback for {job_uuid}");
    let request = requests.into_iter().next().expect("one callback");
    assert_eq!(request.url.as_str(), CALLBACK_URL, "the callback URL");
    let header = |name: &str| {
        request
            .headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned)
    };
    assert_eq!(
        header("content-type").as_deref(),
        Some("application/json"),
        "the default content type"
    );
    assert_eq!(
        header("user-agent"),
        Some(format!("bencher/{}", bencher_json::BENCHER_API_VERSION)),
        "the server's user agent"
    );
    assert_eq!(
        header("authorization").as_deref(),
        Some(CALLBACK_AUTHORIZATION),
        "the customer's header"
    );
    assert_eq!(
        callback_state(server, job_uuid),
        CallbackState::Delivered,
        "the callback is delivered"
    );
    request
}

/// Wait for every delivery, then assert the receiver got nothing and the callback still waits.
pub async fn assert_no_callback(server: &TestServer, job_uuid: JobUuid) {
    server.drain_callbacks().await;
    assert_eq!(server.callback_requests().len(), 0, "no callback is sent");
    assert_eq!(
        callback_state(server, job_uuid),
        CallbackState::Pending,
        "the callback still waits"
    );
}

#[expect(clippy::expect_used, reason = "test helper")]
fn job_id(server: &TestServer, job_uuid: JobUuid) -> JobId {
    schema::job::table
        .filter(schema::job::uuid.eq(job_uuid))
        .select(schema::job::id)
        .first(&mut server.db_conn())
        .expect("Failed to get job ID")
}
