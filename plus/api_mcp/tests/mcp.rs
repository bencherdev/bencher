#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for the MCP Streamable HTTP endpoint (POST /mcp).

use bencher_api_tests::{
    TestOrg, TestProject, TestServer, TestServerOptions, TestUser,
    helpers::{oauth_server, set_job_status},
    oci::create_oci_manifest,
};
use bencher_json::{JobStatus, system::config::JsonRateLimiting};
use bencher_schema::context::RateLimiting;
use http::{StatusCode, header::WWW_AUTHENTICATE};
use serde_json::{Value, json};

const MCP_TOOL_COUNT: usize = 36;
const PROTOCOL_VERSION_2026: &str = "2026-07-28";

/// Build a JSON-RPC POST to /mcp, optionally with a bearer credential.
fn mcp_request(server: &TestServer, bearer: Option<&str>, body: &Value) -> reqwest::RequestBuilder {
    let mut request = server.client.post(server.api_url("/mcp")).json(body);
    if let Some(bearer) = bearer {
        request = request.header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(bearer),
        );
    }
    request
}

/// POST a JSON-RPC message to /mcp, optionally with a bearer credential.
#[expect(clippy::expect_used, reason = "test helper")]
async fn mcp_post(server: &TestServer, bearer: Option<&str>, body: &Value) -> reqwest::Response {
    mcp_request(server, bearer, body)
        .send()
        .await
        .expect("MCP request failed")
}

/// POST a `2026-07-28` JSON-RPC request with the standard headers that version requires.
#[expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test helper; indexing a serde_json value never panics"
)]
async fn mcp_post_2026(
    server: &TestServer,
    bearer: Option<&str>,
    body: &Value,
) -> reqwest::Response {
    let method = body["method"].as_str().expect("Missing JSON-RPC method");
    let mut request = mcp_request(server, bearer, body)
        .header("MCP-Protocol-Version", PROTOCOL_VERSION_2026)
        .header("Mcp-Method", method);
    if let Some(name) = body["params"]["name"].as_str() {
        request = request.header("Mcp-Name", name);
    }
    request.send().await.expect("MCP request failed")
}

/// POST a JSON-RPC request and return the `result` member of the response.
async fn mcp_result(server: &TestServer, bearer: Option<&str>, body: &Value) -> Value {
    let result = json_rpc_result(mcp_post(server, bearer, body).await, body).await;
    assert_output_schema(server, body, &result).await;
    result
}

/// POST a `2026-07-28` JSON-RPC request and return the `result` member of the response.
async fn mcp_result_2026(server: &TestServer, bearer: Option<&str>, body: &Value) -> Value {
    let result = json_rpc_result(mcp_post_2026(server, bearer, body).await, body).await;
    assert_output_schema(server, body, &result).await;
    result
}

/// Assert a successful JSON-RPC response to `body` and return its `result` member.
#[expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test helper; indexing a serde_json object never panics"
)]
async fn json_rpc_result(resp: reqwest::Response, body: &Value) -> Value {
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

/// Validate the `structuredContent` of a successful tool call
/// against the `outputSchema` that `tools/list` declares for the tool.
#[expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test helper; indexing a serde_json value never panics"
)]
async fn assert_output_schema(server: &TestServer, body: &Value, result: &Value) {
    if body["method"] != "tools/call" || result["isError"] == true {
        return;
    }
    let tools_list = json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"});
    let tools = json_rpc_result(mcp_post(server, None, &tools_list).await, &tools_list).await;
    let name = &body["params"]["name"];
    let tool = tools["tools"]
        .as_array()
        .expect("Missing tools array")
        .iter()
        .find(|tool| tool["name"] == *name)
        .expect("Missing tool");
    let content = &result["structuredContent"];
    let Some(schema) = tool.get("outputSchema") else {
        assert!(
            content.is_null(),
            "Structured content without an output schema: {name}"
        );
        return;
    };
    let validator = jsonschema::draft202012::options()
        .should_validate_formats(true)
        .build(schema)
        .expect("Invalid output schema");
    let errors: Vec<String> = validator
        .iter_errors(content)
        .map(|error| error.to_string())
        .collect();
    assert!(
        errors.is_empty(),
        "Structured content of {name} does not match its output schema: {errors:#?}"
    );
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

/// A `2026-07-28` request: self-contained `_meta` instead of an `initialize` handshake.
fn request_2026(method: &str, mut params: Value) -> Value {
    if let Some(params) = params.as_object_mut() {
        params.insert(
            "_meta".to_owned(),
            json!({
                "io.modelcontextprotocol/protocolVersion": PROTOCOL_VERSION_2026,
                "io.modelcontextprotocol/clientInfo": {"name": "test-client", "version": "0.0.0"},
                "io.modelcontextprotocol/clientCapabilities": {}
            }),
        );
    }
    json!({"jsonrpc": "2.0", "id": 3, "method": method, "params": params})
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

// JSON-RPC batches were removed in MCP 2025-06-18,
// so the SDK rejects one as an undecodable body
#[tokio::test]
async fn mcp_batch_rejected() {
    let server = TestServer::new().await;

    let resp = mcp_post(&server, None, &json!([initialize_request("2025-06-18")])).await;

    assert_eq!(resp.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
}

// Methods outside the tools capability return JSON-RPC method not found
#[tokio::test]
async fn mcp_method_not_found() {
    let server = TestServer::new().await;

    for (method, params) in [
        ("resources/list", json!({})),
        ("resources/templates/list", json!({})),
        ("prompts/list", json!({})),
        (
            "completion/complete",
            json!({
                "ref": {"type": "ref/prompt", "name": "benchmark"},
                "argument": {"name": "branch", "value": "main"}
            }),
        ),
    ] {
        let resp = mcp_post(
            &server,
            None,
            &json!({"jsonrpc": "2.0", "id": 7, "method": method, "params": params}),
        )
        .await;

        assert_eq!(resp.status(), StatusCode::OK, "{method}");
        let message: Value = resp.json().await.expect("Invalid JSON-RPC response");
        assert_eq!(message["error"]["code"], -32601, "{method}: {message}");
    }
}

// tools/list returns the full tool surface with schemas, titles, and annotations
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
        // Sign-in requirements are only declared once OAuth is configured
        assert!(tool.get("_meta").is_none(), "{tool}");
        assert_eq!(tool["inputSchema"]["type"], "object", "{tool}");
        assert!(tool["description"].is_string(), "{tool}");
        assert!(tool["title"].is_string(), "{tool}");
        let annotations = &tool["annotations"];
        assert_eq!(annotations["title"], tool["title"], "{tool}");
        for hint in [
            "readOnlyHint",
            "destructiveHint",
            "idempotentHint",
            "openWorldHint",
        ] {
            assert!(annotations[hint].is_boolean(), "Missing {hint}: {tool}");
        }
        assert_eq!(annotations["openWorldHint"], true, "{tool}");
        assert_eq!(
            tool["outputSchema"]["type"] == "object",
            tool["name"] != "perf_image",
            "{tool}"
        );
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

// An invalid bearer credential fails the whole request with 401 whatever its method, like REST,
// rather than being served as anonymous; with OAuth unset there is no challenge to sign in
#[tokio::test]
async fn mcp_invalid_credential() {
    let server = TestServer::new().await;

    for body in [
        call_tool_request("list_projects", &json!({})),
        call_tool_request("no_such_tool", &json!({})),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
        initialize_request("2025-06-18"),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
    ] {
        let resp = mcp_post(&server, Some("bencher_run_invalid"), &body).await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "{body}");
        assert!(
            resp.headers().get(WWW_AUTHENTICATE).is_none(),
            "Unexpected challenge: {body}"
        );
    }
    let discover = request_2026("server/discover", json!({}));
    let resp = mcp_post_2026(&server, Some("bencher_run_invalid"), &discover).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// A user token sees an organization's projects via tools/call
#[tokio::test]
async fn mcp_list_projects_user_token() {
    let server = TestServer::new().await;
    let user = server.signup("MCP User", "mcp-user@bencher.dev").await;
    let org = server.create_org(&user, "MCP Org").await;
    let project = server.create_project(&user, &org, "MCP Project").await;

    let result = mcp_result(
        &server,
        Some(&user.token),
        &call_tool_request("list_projects", &json!({"organization": org.slug})),
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

// A project key lists only its own project, and only under its own organization;
// an anonymous caller lists nothing
#[tokio::test]
async fn mcp_list_projects_project_key() {
    let server = TestServer::new().await;
    let user = server.signup("MCP Lister", "mcp-lister@bencher.dev").await;
    let org = server.create_org(&user, "MCP Lister Org").await;
    let project = server
        .create_project(&user, &org, "MCP Listed Project")
        .await;
    let other = server
        .create_project(&user, &org, "MCP Unlisted Project")
        .await;
    let elsewhere = server.create_org(&user, "MCP Elsewhere Org").await;
    server
        .create_project(&user, &elsewhere, "MCP Elsewhere Project")
        .await;
    let key = server.create_project_key(&user, &project, "mcp-key").await;

    let list = async |bearer: Option<&str>, organization: &TestOrg| {
        let body = call_tool_request("list_projects", &json!({"organization": organization.slug}));
        mcp_result(&server, bearer, &body).await
    };

    let content = tool_content(&list(Some(&user.token), &org).await);
    assert_eq!(content["total_count"], 2, "{content}");

    let content = tool_content(&list(Some(key.key.as_ref()), &org).await);
    let slugs: Vec<&str> = content["data"]
        .as_array()
        .expect("Missing data array")
        .iter()
        .filter_map(|p| p["slug"].as_str())
        .collect();
    let slug: &str = project.slug.as_ref();
    assert_eq!(slugs, [slug], "{content}");
    assert_eq!(content["total_count"], 1, "{content}");
    let other_slug: &str = other.slug.as_ref();
    assert!(!slugs.contains(&other_slug));

    let error = tool_error(&list(Some(key.key.as_ref()), &elsewhere).await);
    assert!(error.starts_with("404"), "Expected 404 error: {error}");

    let error = tool_error(&list(None, &org).await);
    assert!(error.starts_with("401"), "Expected 401 error: {error}");
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

// Every tool with structured output returns results that match its output schema,
// including the types whose JSON is written by hand
#[tokio::test]
async fn mcp_tool_results_match_output_schemas() {
    let server = TestServer::new().await;
    let user = server
        .signup("MCP Schemas", "mcp-schemas@bencher.dev")
        .await;
    let org = server.create_org(&user, "MCP Schemas Org").await;
    let project = server
        .create_project(&user, &org, "MCP Schemas Project")
        .await;

    for (tool, arguments) in seed_every_tool(&server, &user, &org, &project).await {
        let result = mcp_result(
            &server,
            Some(&user.token),
            &call_tool_request(tool, &arguments),
        )
        .await;
        let content = tool_content(&result);
        // A list result only exercises its item schema when it has items
        if tool.starts_with("list_") {
            assert_ne!(content["data"], json!([]), "{tool}: {content}");
        }
    }
}

// With OAuth on, every tool declares the scope it needs and whether an anonymous caller may use it,
// and means it: a tool that declares `noauth` works anonymously on a public project (or, for
// `submit_run`, an unclaimed one) without a challenge, and every other tool asks to sign in
#[tokio::test]
async fn mcp_tools_declare_how_to_sign_in() {
    let server = TestServer::new_with_options(TestServerOptions {
        oauth: Some(oauth_server()),
        ..TestServerOptions::default()
    })
    .await;
    let user = server
        .signup("MCP Signs In", "mcp-signs-in@bencher.dev")
        .await;
    let org = server.create_org(&user, "MCP Signs In Org").await;
    let project = server
        .create_project(&user, &org, "MCP Signs In Project")
        .await;
    let every_tool = seed_every_tool(&server, &user, &org, &project).await;

    let tools_list = json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"});
    let tools = json_rpc_result(mcp_post(&server, None, &tools_list).await, &tools_list).await;
    let tools = tools["tools"].as_array().expect("Missing tools array");
    assert_eq!(tools.len(), every_tool.len());
    for tool in tools {
        let name = tool["name"].as_str().expect("Missing tool name");
        let schemes = tool["_meta"]["securitySchemes"]
            .as_array()
            .expect("Missing security schemes");
        let scope = if tool["annotations"]["readOnlyHint"] == true {
            "projects:read"
        } else {
            "projects:write"
        };
        assert!(
            schemes.contains(&json!({"type": "oauth2", "scopes": [scope]})),
            "{name}: {schemes:?}"
        );

        let arguments = if name == "submit_run" {
            json!({
                "project": "mcp-unclaimed",
                "context": {"bencher.dev/v0/repo/name": "mcp-unclaimed"},
                "start_time": "2025-01-01T00:00:00Z",
                "end_time": "2025-01-01T00:01:00Z",
                "results": ["{\"mcp_bench\": {\"latency\": {\"value\": 88.0}}}"]
            })
        } else {
            every_tool
                .iter()
                .find_map(|(tool, arguments)| (*tool == name).then(|| arguments.clone()))
                .expect("Missing arguments")
        };
        let body = call_tool_request(name, &arguments);
        let resp = mcp_post(&server, None, &body).await;
        if schemes.contains(&json!({"type": "noauth"})) {
            let result = json_rpc_result(resp, &body).await;
            assert_ne!(result["isError"], true, "{name}: {result}");
            assert!(result.get("_meta").is_none(), "{name}: {result}");
        } else {
            assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "{name}");
            let challenge = resp
                .headers()
                .get(WWW_AUTHENTICATE)
                .and_then(|value| value.to_str().ok())
                .expect("Missing challenge");
            assert!(
                challenge.starts_with(r#"Bearer resource_metadata=""#),
                "{name}: {challenge}"
            );
        }
    }
}

/// Seed a project with one of everything the tools read, through the tools themselves,
/// and return arguments that call every tool on it.
#[expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::too_many_lines,
    reason = "test helper with one call per tool; indexing a serde_json value never panics"
)]
async fn seed_every_tool(
    server: &TestServer,
    user: &TestUser,
    org: &TestOrg,
    project: &TestProject,
) -> Vec<(&'static str, Value)> {
    let project_slug: &str = project.slug.as_ref();
    let call = async |name: &str, mut arguments: Value| {
        arguments["project"] = json!(project_slug);
        let request = call_tool_request(name, &arguments);
        tool_content(&mcp_result(server, Some(&user.token), &request).await)
    };

    // The first user to sign up is the server admin, who registers hardware specs
    let spec = post_spec(
        server,
        user,
        json!({"name": "Bare Metal", "fallback": true}),
    )
    .await;
    let sandboxed = post_spec(
        server,
        user,
        json!({"name": "Sandboxed", "sandbox": "firecracker"}),
    )
    .await;
    let branch = call("create_branch", json!({"name": "main"})).await;
    let testbed = call(
        "create_testbed",
        json!({"name": "localhost", "spec": spec["uuid"]}),
    )
    .await;
    // A spec without a sandbox rides along as `"sandbox": null` in every testbed
    assert!(testbed["spec"]["sandbox"].is_null(), "{testbed}");
    call(
        "create_testbed",
        json!({"name": "sandboxed", "spec": sandboxed["uuid"]}),
    )
    .await;
    let measure = call("create_measure", json!({"name": "latency", "units": "ns"})).await;
    let threshold = call(
        "create_threshold",
        json!({
            "branch": branch["uuid"],
            "testbed": testbed["uuid"],
            "measure": measure["uuid"],
            "parameters": [{"threads": 4}],
            "test": "static",
            "upper_boundary": 100.0
        }),
    )
    .await;
    let run = |day: u8, value: f64| {
        let results = json!({
            "mcp_bench": [{"parameters": {"threads": 4}, "measures": {"latency": {"value": value}}}]
        });
        json!({
            "branch": branch["uuid"],
            "testbed": testbed["uuid"],
            "start_time": format!("2025-01-0{day}T00:00:00Z"),
            "end_time": format!("2025-01-0{day}T00:01:00Z"),
            "bmf_version": 1,
            "results": [results.to_string()]
        })
    };
    // A threshold only checks a metric once it has an earlier one to compare
    call("create_report", run(1, 10.0)).await;
    let report = call("submit_run", run(2, 1000.0)).await;
    let result = &report["results"][0][0];
    let benchmark = &result["benchmark"];
    let alerts = call("list_alerts", json!({})).await;
    let alert = &alerts["data"][0];
    assert!(alert.is_object(), "Expected an alert: {alerts}");

    let plot = server
        .client
        .post(server.api_url(&format!("/v0/projects/{project_slug}/plots")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&json!({
            "title": "MCP Plot",
            "lower_value": false,
            "upper_value": false,
            "lower_boundary": false,
            "upper_boundary": true,
            "x_axis": "date_time",
            "layout": "stacked",
            "window": 2_592_000,
            "branches": [branch["uuid"]],
            "testbeds": [testbed["uuid"]],
            "benchmarks": [benchmark["uuid"]],
            "parameters": [{"threads": 4}],
            "measures": [measure["uuid"]],
            "metrics": ["p99", "value"],
            "hidden": ["1a2b3c4d5e6"],
            "focus": "0123456789abcdef"
        }))
        .send()
        .await
        .expect("Plot request failed");
    assert_eq!(plot.status(), StatusCode::CREATED, "Failed to create plot");
    let plot: Value = plot.json().await.expect("Invalid plot");

    let mut job_run = run(3, 10.0);
    job_run["job"] = json!({
        "image": push_image(server, user, project).await,
        "entrypoint": ["/bin/sh"],
        "cmd": ["-c", "bench"],
        "env": {"THREADS": "4"},
        "file_paths": ["/out/results.json"],
        "callback": {"url": "https://example.com/hook", "headers": {"X-Bench": "1"}}
    });
    let job = call("submit_run", job_run).await["job"].clone();
    let pending = call("view_job", json!({"job": job})).await;
    assert!(pending["callback"].is_object(), "{pending}");
    let job_uuid = serde_json::from_value(job.clone()).expect("Invalid job UUID");
    set_job_status(server, job_uuid, JobStatus::Failed);
    let output = serde_json::from_value(json!({
        "results": [{"exit_code": 1, "stdout": "out", "stderr": "err", "output": {"/out/results.json": "{}"}}],
        "error": "Benchmark failed"
    }))
    .expect("Invalid job output");
    server
        .context()
        .oci_storage()
        .job_output()
        .put(project.uuid, job_uuid, &output)
        .await
        .expect("Failed to store job output");
    let failed = call("view_job", json!({"job": job})).await;
    assert!(failed["output"].is_object(), "{failed}");

    let perf = json!({
        "project": project_slug,
        "branches": branch["uuid"],
        "testbeds": testbed["uuid"],
        "benchmarks": benchmark["uuid"],
        "measures": measure["uuid"]
    });
    let mut every_tool = vec![
        ("list_projects", json!({"organization": org.slug})),
        ("view_project", json!({})),
        ("list_reports", json!({})),
        ("view_report", json!({"report": report["uuid"]})),
        ("list_branches", json!({})),
        ("view_branch", json!({"branch": branch["uuid"]})),
        ("list_testbeds", json!({})),
        ("view_testbed", json!({"testbed": testbed["uuid"]})),
        ("list_benchmarks", json!({})),
        ("view_benchmark", json!({"benchmark": benchmark["uuid"]})),
        ("list_measures", json!({})),
        ("view_measure", json!({"measure": measure["uuid"]})),
        ("list_thresholds", json!({})),
        ("view_threshold", json!({"threshold": threshold["uuid"]})),
        ("list_alerts", json!({})),
        ("view_alert", json!({"alert": alert["uuid"]})),
        (
            "view_metric",
            json!({"metric": result["measures"][0]["metrics"][0]["uuid"]}),
        ),
        ("query_perf", perf.clone()),
        ("perf_image", perf),
        ("list_plots", json!({})),
        ("view_plot", json!({"plot": plot["uuid"]})),
        ("list_jobs", json!({})),
        ("view_job", json!({"job": job})),
        (
            "create_branch",
            json!({"name": "feature", "start_point": {"branch": branch["uuid"]}}),
        ),
        (
            "update_branch",
            json!({"branch": branch["uuid"], "name": "renamed"}),
        ),
        ("create_testbed", json!({"name": "other"})),
        (
            "update_testbed",
            json!({"testbed": testbed["uuid"], "name": "renamed"}),
        ),
        ("create_benchmark", json!({"name": "mcp_other"})),
        (
            "update_benchmark",
            json!({"benchmark": benchmark["uuid"], "name": "renamed"}),
        ),
        (
            "create_measure",
            json!({"name": "throughput", "units": "ops"}),
        ),
        (
            "update_measure",
            json!({"measure": measure["uuid"], "name": "renamed"}),
        ),
        (
            "create_threshold",
            json!({
                "branch": branch["uuid"],
                "testbed": testbed["uuid"],
                "measure": measure["uuid"],
                "parameters": [{"threads": 8}],
                "test": "static",
                "upper_boundary": 100.0
            }),
        ),
        (
            "update_threshold",
            json!({"threshold": threshold["uuid"], "test": "static", "upper_boundary": 200.0}),
        ),
        (
            "update_alert",
            json!({"alert": alert["uuid"], "status": "dismissed"}),
        ),
        ("create_report", run(4, 10.0)),
        ("submit_run", run(5, 10.0)),
    ];
    for (tool, arguments) in &mut every_tool {
        if *tool != "list_projects" {
            arguments["project"] = json!(project_slug);
        }
    }
    every_tool
}

/// Register a hardware spec, which only the server admin can do.
#[expect(clippy::expect_used, reason = "test helper")]
async fn post_spec(server: &TestServer, admin: &TestUser, spec: Value) -> Value {
    let mut body = json!({
        "os": "linux",
        "architecture": "x86_64",
        "cpu": 2,
        "memory": 0x0001_0000_0000i64,
        "disk": 0x0002_8000_0000i64
    });
    if let (Some(body), Some(spec)) = (body.as_object_mut(), spec.as_object()) {
        body.extend(spec.clone());
    }
    let resp = server
        .client
        .post(server.api_url("/v0/specs"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Spec request failed");
    assert_eq!(resp.status(), StatusCode::CREATED, "Failed to create spec");
    resp.json().await.expect("Invalid spec")
}

/// Push an image to the project's registry and return its reference.
#[expect(clippy::expect_used, reason = "test helper")]
async fn push_image(server: &TestServer, user: &TestUser, project: &TestProject) -> String {
    let project_slug: &str = project.slug.as_ref();
    let token = server.oci_push_token(user, project);
    let config = server.upload_blob(project_slug, &token, b"config").await;
    let layer = server.upload_blob(project_slug, &token, b"layer").await;
    let resp = server
        .client
        .put(server.api_url(&format!("/v2/{project_slug}/manifests/v1")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&token),
        )
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/vnd.oci.image.manifest.v1+json",
        )
        .body(create_oci_manifest(&config, &layer))
        .send()
        .await
        .expect("Manifest push failed");
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "Failed to push manifest"
    );
    format!("localhost/{project_slug}:v1")
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

// Content negotiation headers never fail a request: every body is read as JSON
#[tokio::test]
async fn mcp_content_type_not_required() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .post(server.api_url("/mcp"))
        .header(reqwest::header::CONTENT_TYPE, "text/plain")
        .body(json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}).to_string())
        .send()
        .await
        .expect("MCP request failed");

    assert_eq!(resp.status(), StatusCode::OK);
}

// Legacy clients are answered with the protocol version they initialize with
#[tokio::test]
async fn mcp_initialize_legacy_versions() {
    let server = TestServer::new().await;

    for version in ["2025-03-26", "2025-11-25"] {
        let result = mcp_result(&server, None, &initialize_request(version)).await;
        assert_eq!(result["protocolVersion"], version);
    }
}

// `2026-07-28` discovery lists the supported protocol versions and the server's capabilities
#[tokio::test]
async fn mcp_discover() {
    let server = TestServer::new().await;

    let result = mcp_result_2026(&server, None, &request_2026("server/discover", json!({}))).await;

    assert_eq!(
        result["supportedVersions"],
        json!(["2025-03-26", "2025-06-18", "2025-11-25", "2026-07-28"])
    );
    assert!(result["capabilities"]["tools"].is_object());
    assert!(result["instructions"].is_string());
    assert_eq!(
        result["_meta"]["io.modelcontextprotocol/serverInfo"]["name"],
        "bencher"
    );
}

// A `2026-07-28` client lists tools without an `initialize` handshake
#[tokio::test]
async fn mcp_tools_list_2026() {
    let server = TestServer::new().await;

    let result = mcp_result_2026(&server, None, &request_2026("tools/list", json!({}))).await;

    assert_eq!(
        result["tools"].as_array().map(Vec::len),
        Some(MCP_TOOL_COUNT)
    );
}

// A `2026-07-28` tool call with a user token sees the user's projects
#[tokio::test]
async fn mcp_list_projects_2026_user_token() {
    let server = TestServer::new().await;
    let user = server.signup("MCP Modern", "mcp-modern@bencher.dev").await;
    let org = server.create_org(&user, "MCP Modern Org").await;
    let project = server
        .create_project(&user, &org, "MCP Modern Project")
        .await;

    let result = mcp_result_2026(
        &server,
        Some(&user.token),
        &request_2026(
            "tools/call",
            json!({"name": "list_projects", "arguments": {"organization": org.slug}}),
        ),
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
}

// An anonymous `2026-07-28` tool call can read a public project
#[tokio::test]
async fn mcp_anonymous_public_project_2026() {
    let server = TestServer::new().await;
    let user = server
        .signup("MCP Modern Anon", "mcp-modern-anon@bencher.dev")
        .await;
    let org = server.create_org(&user, "MCP Modern Anon Org").await;
    let project = server
        .create_project(&user, &org, "MCP Modern Anon Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let result = mcp_result_2026(
        &server,
        None,
        &request_2026(
            "tools/call",
            json!({"name": "view_project", "arguments": {"project": project_slug}}),
        ),
    )
    .await;

    let content = tool_content(&result);
    assert_eq!(content["slug"], project_slug);
}

// A protocol version header that disagrees with the body is rejected,
// and the SDK's error response still carries the CORS headers
#[tokio::test]
async fn mcp_protocol_version_header_mismatch() {
    let server = TestServer::new().await;

    let resp = mcp_request(&server, None, &initialize_request("2025-03-26"))
        .header("MCP-Protocol-Version", "2025-06-18")
        .send()
        .await
        .expect("MCP request failed");

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert_cors_headers(resp.headers());
}

// Responses and the preflight carry the CORS headers browser clients need
#[tokio::test]
async fn mcp_cors_headers() {
    let server = TestServer::new().await;

    let resp = mcp_post(
        &server,
        None,
        &json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_cors_headers(resp.headers());

    let resp = server
        .client
        .request(reqwest::Method::OPTIONS, server.api_url("/mcp"))
        .send()
        .await
        .expect("MCP preflight failed");
    assert!(resp.status().is_success(), "Preflight failed: {resp:?}");
    assert_cors_headers(resp.headers());
}

// Neither the `Host` nor the `Origin` header is validated:
// the server is reached through public hostnames by cross-origin clients
#[tokio::test]
async fn mcp_public_host_and_cross_origin() {
    let server = TestServer::new().await;

    let resp = mcp_request(
        &server,
        None,
        &json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
    )
    .header(reqwest::header::HOST, "api.bencher.dev")
    .header(reqwest::header::ORIGIN, "https://client.example")
    .send()
    .await
    .expect("MCP request failed");

    assert_eq!(resp.status(), StatusCode::OK);
}

// Anonymous requests are throttled per IP address before the SDK handles them,
// while signed-in requests are charged to their credential whatever they are
#[tokio::test]
async fn mcp_public_rate_limit() {
    let server = rate_limited_server(json!({"public": {"requests": {"minute": 1}}})).await;
    let user = server
        .signup("MCP Throttled", "mcp-throttled@bencher.dev")
        .await;
    let org = server.create_org(&user, "MCP Throttled Org").await;
    let project = server
        .create_project(&user, &org, "MCP Throttled Project")
        .await;
    let key = server.create_project_key(&user, &project, "mcp-key").await;

    let post_from_ip = async |bearer: Option<&str>, body: Value| {
        mcp_request(&server, bearer, &body)
            .header("X-Forwarded-For", "203.0.113.7")
            .send()
            .await
            .expect("MCP request failed")
            .status()
    };
    let tools_list = json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"});

    assert_eq!(post_from_ip(None, tools_list.clone()).await, StatusCode::OK);
    assert_eq!(
        post_from_ip(None, tools_list.clone()).await,
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(
        post_from_ip(
            None,
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"})
        )
        .await,
        StatusCode::TOO_MANY_REQUESTS
    );
    // With OAuth unset, an anonymous tool call over the limit is refused, not asked to sign in
    assert_eq!(
        post_from_ip(None, call_tool_request("list_projects", &json!({}))).await,
        StatusCode::TOO_MANY_REQUESTS
    );
    let bearers: [&str; 2] = [&user.token, key.key.as_ref()];
    for bearer in bearers {
        assert_eq!(
            post_from_ip(Some(bearer), call_tool_request("list_projects", &json!({}))).await,
            StatusCode::OK
        );
        assert_eq!(
            post_from_ip(Some(bearer), tools_list.clone()).await,
            StatusCode::OK
        );
    }
    // Messages the SDK does not dispatch as a tool call are charged to their credential too
    assert_eq!(
        post_from_ip(
            Some(&user.token),
            json!({
                "jsonrpc": "2.0",
                "id": 42,
                "method": "tools/call",
                "params": {"name": "list_projects", "arguments": 5}
            })
        )
        .await,
        StatusCode::OK
    );
    assert_eq!(
        post_from_ip(
            Some(&user.token),
            json!({
                "jsonrpc": "2.0",
                "method": "tools/call",
                "params": {"name": "list_projects", "arguments": {}}
            })
        )
        .await,
        StatusCode::ACCEPTED
    );
}

// An anonymous tool call is charged to its address once, like every other anonymous message
#[tokio::test]
async fn mcp_anonymous_charged_once() {
    let server = rate_limited_server(json!({"public": {"requests": {"minute": 2}}})).await;

    let mut statuses = Vec::new();
    for _ in 0..3 {
        let status = mcp_request(
            &server,
            None,
            &call_tool_request("list_projects", &json!({})),
        )
        .header("X-Forwarded-For", "203.0.113.8")
        .send()
        .await
        .expect("MCP request failed")
        .status();
        statuses.push(status);
    }
    assert_eq!(
        statuses,
        [
            StatusCode::OK,
            StatusCode::OK,
            StatusCode::TOO_MANY_REQUESTS
        ]
    );
}

// Signed-in messages are charged to their credential whatever their method: a user's to the user,
// and a project key's to its project, where a tool call is charged once, by the tool
#[tokio::test]
async fn mcp_signed_in_charged_to_credential() {
    let server = rate_limited_server(json!({
        "user": {"requests": {"minute": 4}},
        "project": {"requests": {"minute": 4}}
    }))
    .await;
    let reader = server.signup("MCP Reader", "mcp-reader@bencher.dev").await;
    let owner = server.signup("MCP Owner", "mcp-owner@bencher.dev").await;
    let org = server.create_org(&owner, "MCP Charged Org").await;
    let project = server
        .create_project(&owner, &org, "MCP Charged Project")
        .await;
    // Creating the key is the project's first charged request
    let key = server.create_project_key(&owner, &project, "mcp-key").await;
    let tools_list = json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"});

    let mut statuses = Vec::new();
    for _ in 0..5 {
        statuses.push(
            mcp_post(&server, Some(&reader.token), &tools_list)
                .await
                .status(),
        );
    }
    assert_eq!(
        statuses,
        [
            StatusCode::OK,
            StatusCode::OK,
            StatusCode::OK,
            StatusCode::OK,
            StatusCode::TOO_MANY_REQUESTS
        ]
    );

    let key: &str = key.key.as_ref();
    for _ in 0..2 {
        assert_eq!(
            mcp_post(&server, Some(key), &tools_list).await.status(),
            StatusCode::OK
        );
    }
    let view_project = call_tool_request("view_project", &json!({"project": project.slug}));
    tool_content(&mcp_result(&server, Some(key), &view_project).await);
    assert_eq!(
        mcp_post(&server, Some(key), &tools_list).await.status(),
        StatusCode::TOO_MANY_REQUESTS
    );
}

#[expect(clippy::expect_used, reason = "test helper parsing fixed rate limits")]
async fn rate_limited_server(rate_limiting: Value) -> TestServer {
    let rate_limiting: JsonRateLimiting =
        serde_json::from_value(rate_limiting).expect("Invalid rate limiting");
    TestServer::new_with_rate_limiting(RateLimiting::from(rate_limiting)).await
}

/// Assert the CORS headers a browser based MCP client needs.
#[expect(clippy::expect_used, reason = "test helper")]
fn assert_cors_headers(headers: &reqwest::header::HeaderMap) {
    assert_eq!(
        headers.get(reqwest::header::ACCESS_CONTROL_ALLOW_ORIGIN),
        Some(&reqwest::header::HeaderValue::from_static("*")),
        "Missing allowed origin"
    );
    let allow_headers = headers
        .get(reqwest::header::ACCESS_CONTROL_ALLOW_HEADERS)
        .and_then(|value| value.to_str().ok())
        .expect("Missing allowed headers");
    for header in [
        "Content-Type",
        "Authorization",
        "Mcp-Protocol-Version",
        "Mcp-Method",
        "Mcp-Name",
    ] {
        assert!(
            allow_headers.contains(header),
            "Missing allowed header {header}: {allow_headers}"
        );
    }
}
