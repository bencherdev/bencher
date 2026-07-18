#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for the MCP Streamable HTTP endpoint (POST /mcp).

use bencher_api_tests::TestServer;
use http::StatusCode;
use serde_json::{Value, json};

const MCP_TOOL_COUNT: usize = 36;

/// POST a JSON-RPC message to /mcp, optionally with a bearer credential.
#[expect(clippy::expect_used, reason = "test helper")]
async fn mcp_post(server: &TestServer, bearer: Option<&str>, body: &Value) -> reqwest::Response {
    let mut request = server.client.post(server.api_url("/mcp")).json(body);
    if let Some(bearer) = bearer {
        request = request.header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(bearer),
        );
    }
    request.send().await.expect("MCP request failed")
}

/// POST a JSON-RPC request and return the `result` member of the response.
#[expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test helper; indexing a serde_json object never panics"
)]
async fn mcp_result(server: &TestServer, bearer: Option<&str>, body: &Value) -> Value {
    let resp = mcp_post(server, bearer, body).await;
    assert_eq!(resp.status(), StatusCode::OK, "Expected 200 OK");
    let message: Value = resp.json().await.expect("Invalid JSON-RPC response");
    assert_eq!(message["jsonrpc"], "2.0", "Missing JSON-RPC version");
    assert_eq!(message["id"], body["id"], "Response id does not match");
    assert!(
        message.get("error").is_none(),
        "Unexpected JSON-RPC error: {message}"
    );
    message["result"].clone()
}

fn initialize_request(protocol_version: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": protocol_version,
            "capabilities": {},
            "clientInfo": {"name": "test-client", "version": "0.0.0"}
        }
    })
}

fn call_tool_request(name: &str, arguments: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": 42,
        "method": "tools/call",
        "params": {"name": name, "arguments": arguments}
    })
}

/// Assert a successful tool call and return its `structuredContent`.
fn tool_content(result: &Value) -> Value {
    assert_ne!(result["isError"], true, "Unexpected tool error: {result}");
    result["structuredContent"].clone()
}

/// Assert a failed tool call and return its text content.
#[expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test helper; indexing a serde_json value never panics"
)]
fn tool_error(result: &Value) -> String {
    assert_eq!(result["isError"], true, "Expected tool error: {result}");
    result["content"][0]["text"]
        .as_str()
        .expect("Missing tool error text")
        .to_owned()
}

// Initialize negotiates a supported protocol version and reports tools capability
#[tokio::test]
async fn mcp_initialize() {
    let server = TestServer::new().await;

    let result = mcp_result(&server, None, &initialize_request("2025-06-18")).await;

    assert_eq!(result["protocolVersion"], "2025-06-18");
    assert!(result["capabilities"]["tools"].is_object());
    assert_eq!(result["serverInfo"]["name"], "bencher");
    assert!(result["instructions"].is_string());
}

// An unsupported protocol version gets the server's latest supported version
#[tokio::test]
async fn mcp_initialize_unsupported_version() {
    let server = TestServer::new().await;

    let result = mcp_result(&server, None, &initialize_request("2024-11-05")).await;

    assert_eq!(result["protocolVersion"], "2025-11-25");
}

// Notifications are accepted and discarded with 202 Accepted
#[tokio::test]
async fn mcp_notification_accepted() {
    let server = TestServer::new().await;

    let resp = mcp_post(
        &server,
        None,
        &json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::ACCEPTED);
}

// JSON-RPC batches were removed in MCP 2025-06-18
#[tokio::test]
async fn mcp_batch_rejected() {
    let server = TestServer::new().await;

    let resp = mcp_post(&server, None, &json!([initialize_request("2025-06-18")])).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let message: Value = resp.json().await.expect("Invalid JSON-RPC response");
    assert_eq!(message["error"]["code"], -32600);
}

// Unknown methods return JSON-RPC method not found
#[tokio::test]
async fn mcp_method_not_found() {
    let server = TestServer::new().await;

    let resp = mcp_post(
        &server,
        None,
        &json!({"jsonrpc": "2.0", "id": 7, "method": "resources/list"}),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::OK);
    let message: Value = resp.json().await.expect("Invalid JSON-RPC response");
    assert_eq!(message["error"]["code"], -32601);
}

// tools/list returns the full tool surface with schemas
#[tokio::test]
async fn mcp_tools_list() {
    let server = TestServer::new().await;

    let result = mcp_result(
        &server,
        None,
        &json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
    )
    .await;

    let tools = result["tools"].as_array().expect("Missing tools array");
    assert_eq!(tools.len(), MCP_TOOL_COUNT);
    let names: Vec<&str> = tools
        .iter()
        .map(|tool| tool["name"].as_str().expect("Missing tool name"))
        .collect();
    for name in ["submit_run", "query_perf", "list_alerts", "view_project"] {
        assert!(names.contains(&name), "Missing tool: {name}");
    }
    for tool in tools {
        assert_eq!(tool["inputSchema"]["type"], "object", "{tool}");
        assert!(tool["description"].is_string(), "{tool}");
    }
}

// An unknown tool is a JSON-RPC invalid params error
#[tokio::test]
async fn mcp_unknown_tool() {
    let server = TestServer::new().await;

    let resp = mcp_post(
        &server,
        None,
        &call_tool_request("no_such_tool", &json!({})),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::OK);
    let message: Value = resp.json().await.expect("Invalid JSON-RPC response");
    assert_eq!(message["error"]["code"], -32602);
}

// An invalid bearer credential fails the whole request with 401, like REST
#[tokio::test]
async fn mcp_invalid_credential() {
    let server = TestServer::new().await;

    let resp = mcp_post(
        &server,
        Some("bencher_run_invalid"),
        &call_tool_request("list_projects", &json!({})),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// A user token sees their projects via tools/call
#[tokio::test]
async fn mcp_list_projects_user_token() {
    let server = TestServer::new().await;
    let user = server.signup("MCP User", "mcp-user@bencher.dev").await;
    let org = server.create_org(&user, "MCP Org").await;
    let project = server.create_project(&user, &org, "MCP Project").await;

    let result = mcp_result(
        &server,
        Some(&user.token),
        &call_tool_request("list_projects", &json!({})),
    )
    .await;

    let content = tool_content(&result);
    let slugs: Vec<&str> = content["data"]
        .as_array()
        .expect("Missing data array")
        .iter()
        .filter_map(|p| p["slug"].as_str())
        .collect();
    let slug: &str = project.slug.as_ref();
    assert!(slugs.contains(&slug), "Missing project: {slugs:?}");
    assert!(content["total_count"].is_number());
}

// A project key can view its own project but not another one
#[tokio::test]
async fn mcp_project_key_scoping() {
    let server = TestServer::new().await;
    let user = server.signup("MCP Keyed", "mcp-keyed@bencher.dev").await;
    let org = server.create_org(&user, "MCP Keyed Org").await;
    let project = server
        .create_project(&user, &org, "MCP Keyed Project")
        .await;
    let other = server
        .create_project(&user, &org, "MCP Other Project")
        .await;
    let key = server.create_project_key(&user, &project, "mcp-key").await;

    let project_slug: &str = project.slug.as_ref();
    let result = mcp_result(
        &server,
        Some(key.key.as_ref()),
        &call_tool_request("view_project", &json!({"project": project_slug})),
    )
    .await;
    let content = tool_content(&result);
    assert_eq!(content["slug"], project_slug);

    let other_slug: &str = other.slug.as_ref();
    let result = mcp_result(
        &server,
        Some(key.key.as_ref()),
        &call_tool_request("view_project", &json!({"project": other_slug})),
    )
    .await;
    let error = tool_error(&result);
    assert!(error.starts_with("403"), "Expected 403 error: {error}");
}

// A project key can create a testbed but not rename one
#[tokio::test]
async fn mcp_project_key_create_but_not_rename() {
    let server = TestServer::new().await;
    let user = server.signup("MCP Rename", "mcp-rename@bencher.dev").await;
    let org = server.create_org(&user, "MCP Rename Org").await;
    let project = server
        .create_project(&user, &org, "MCP Rename Project")
        .await;
    let key = server.create_project_key(&user, &project, "mcp-key").await;
    let project_slug: &str = project.slug.as_ref();

    let result = mcp_result(
        &server,
        Some(key.key.as_ref()),
        &call_tool_request(
            "create_testbed",
            &json!({"project": project_slug, "name": "MCP Testbed"}),
        ),
    )
    .await;
    let content = tool_content(&result);
    let testbed_slug = content["slug"].as_str().expect("Missing testbed slug");

    let result = mcp_result(
        &server,
        Some(key.key.as_ref()),
        &call_tool_request(
            "update_testbed",
            &json!({
                "project": project_slug,
                "testbed": testbed_slug,
                "name": "Renamed Testbed"
            }),
        ),
    )
    .await;
    let error = tool_error(&result);
    assert!(error.starts_with("403"), "Expected 403 error: {error}");
}

// Anonymous requests can read a public project, mirroring the REST API
#[tokio::test]
async fn mcp_anonymous_public_project() {
    let server = TestServer::new().await;
    let user = server.signup("MCP Anon", "mcp-anon@bencher.dev").await;
    let org = server.create_org(&user, "MCP Anon Org").await;
    let project = server.create_project(&user, &org, "MCP Anon Project").await;

    let project_slug: &str = project.slug.as_ref();
    let result = mcp_result(
        &server,
        None,
        &call_tool_request("view_project", &json!({"project": project_slug})),
    )
    .await;
    let content = tool_content(&result);
    assert_eq!(content["slug"], project_slug);
}

// A project key can submit raw benchmark results end-to-end:
// the server parses them and creates a report visible via list_reports
#[tokio::test]
async fn mcp_submit_run_project_key() {
    let server = TestServer::new().await;
    let user = server.signup("MCP Runner", "mcp-runner@bencher.dev").await;
    let org = server.create_org(&user, "MCP Runner Org").await;
    let project = server
        .create_project(&user, &org, "MCP Runner Project")
        .await;
    let key = server.create_project_key(&user, &project, "mcp-key").await;
    let project_slug: &str = project.slug.as_ref();

    let result = mcp_result(
        &server,
        Some(key.key.as_ref()),
        &call_tool_request(
            "submit_run",
            &json!({
                "start_time": "2025-01-01T00:00:00Z",
                "end_time": "2025-01-01T00:01:00Z",
                "results": ["{\"mcp_bench\": {\"latency\": {\"value\": 88.0}}}"]
            }),
        ),
    )
    .await;
    let report = tool_content(&result);
    assert_eq!(report["project"]["slug"], project_slug);

    let result = mcp_result(
        &server,
        Some(key.key.as_ref()),
        &call_tool_request("list_reports", &json!({"project": project_slug})),
    )
    .await;
    let content = tool_content(&result);
    assert_eq!(content["total_count"], 1);
}

// Anonymous requests cannot read a private project
#[tokio::test]
async fn mcp_anonymous_private_project() {
    use bencher_json::project::Visibility;
    use bencher_schema::schema;
    use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};

    let server = TestServer::new().await;
    let user = server
        .signup("MCP Private", "mcp-private@bencher.dev")
        .await;
    let org = server.create_org(&user, "MCP Private Org").await;
    let project = server
        .create_project(&user, &org, "MCP Private Project")
        .await;

    {
        let mut conn = server.db_conn();
        diesel::update(schema::project::table.filter(schema::project::uuid.eq(project.uuid)))
            .set(schema::project::visibility.eq(Visibility::Private))
            .execute(&mut conn)
            .expect("Failed to update project visibility");
    }

    let project_slug: &str = project.slug.as_ref();
    let result = mcp_result(
        &server,
        None,
        &call_tool_request("view_project", &json!({"project": project_slug})),
    )
    .await;
    let error = tool_error(&result);
    assert!(error.starts_with("404"), "Expected 404 error: {error}");
}

// Malformed tool arguments are a tool error, not a protocol error
#[tokio::test]
async fn mcp_invalid_tool_arguments() {
    let server = TestServer::new().await;

    let result = mcp_result(
        &server,
        None,
        &call_tool_request("view_project", &json!({"not_a_field": true})),
    )
    .await;

    let error = tool_error(&result);
    assert!(error.starts_with("400"), "Expected 400 error: {error}");
}
