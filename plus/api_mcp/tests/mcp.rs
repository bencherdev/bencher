#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for the MCP Streamable HTTP endpoint (POST /mcp).

use bencher_api_tests::{
    TestProject, TestServer, TestUser, helpers::set_job_status, oci::create_oci_manifest,
};
use bencher_json::JobStatus;
use http::StatusCode;
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
        assert_eq!(tool["inputSchema"]["type"], "object", "{tool}");
        assert!(tool["description"].is_string(), "{tool}");
        assert!(tool["title"].is_string(), "{tool}");
        let annotations = &tool["annotations"];
        assert_eq!(annotations["title"], tool["title"], "{tool}");
        for hint in ["readOnlyHint", "destructiveHint", "openWorldHint"] {
            assert!(annotations[hint].is_boolean(), "Missing {hint}: {tool}");
        }
        assert_eq!(
            annotations["idempotentHint"].is_boolean(),
            annotations["readOnlyHint"] == false,
            "{tool}"
        );
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

// An unknown tool is rejected before credentials are resolved
#[tokio::test]
async fn mcp_unknown_tool_invalid_credential() {
    let server = TestServer::new().await;

    let resp = mcp_post(
        &server,
        Some("bencher_run_invalid"),
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

// Every tool with structured output returns results that match its output schema,
// including the types whose JSON is written by hand
#[tokio::test]
#[expect(clippy::too_many_lines, reason = "one call per tool")]
async fn mcp_tool_results_match_output_schemas() {
    let server = TestServer::new().await;
    let user = server
        .signup("MCP Schemas", "mcp-schemas@bencher.dev")
        .await;
    let org = server.create_org(&user, "MCP Schemas Org").await;
    let project = server
        .create_project(&user, &org, "MCP Schemas Project")
        .await;
    let project_slug: &str = project.slug.as_ref();
    let call = async |name: &str, mut arguments: Value| {
        arguments["project"] = json!(project_slug);
        let request = call_tool_request(name, &arguments);
        tool_content(&mcp_result(&server, Some(&user.token), &request).await)
    };

    // The first user to sign up is the server admin, who registers hardware specs
    let spec = post_spec(
        &server,
        &user,
        json!({"name": "Bare Metal", "fallback": true}),
    )
    .await;
    let sandboxed = post_spec(
        &server,
        &user,
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
    assert_eq!(plot.status(), StatusCode::CREATED);
    let plot: Value = plot.json().await.expect("Invalid plot");

    let mut job_run = run(3, 10.0);
    job_run["job"] = json!({
        "image": push_image(&server, &user, &project).await,
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
    set_job_status(&server, job_uuid, JobStatus::Failed);
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

    for (tool, arguments) in [
        ("view_report", json!({"report": report["uuid"]})),
        ("view_alert", json!({"alert": alert["uuid"]})),
        (
            "update_alert",
            json!({"alert": alert["uuid"], "status": "dismissed"}),
        ),
        (
            "view_metric",
            json!({"metric": result["measures"][0]["metrics"][0]["uuid"]}),
        ),
        (
            "query_perf",
            json!({
                "branches": branch["uuid"],
                "testbeds": testbed["uuid"],
                "benchmarks": benchmark["uuid"],
                "measures": measure["uuid"]
            }),
        ),
        ("view_threshold", json!({"threshold": threshold["uuid"]})),
        (
            "update_threshold",
            json!({"threshold": threshold["uuid"], "test": "static", "upper_boundary": 200.0}),
        ),
        ("view_plot", json!({"plot": plot["uuid"]})),
        ("create_benchmark", json!({"name": "mcp_other"})),
        (
            "create_branch",
            json!({"name": "feature", "start_point": {"branch": branch["uuid"]}}),
        ),
        ("view_branch", json!({"branch": branch["uuid"]})),
        (
            "update_branch",
            json!({"branch": branch["uuid"], "name": "renamed"}),
        ),
        ("view_testbed", json!({"testbed": testbed["uuid"]})),
        (
            "update_testbed",
            json!({"testbed": testbed["uuid"], "name": "renamed"}),
        ),
        ("view_benchmark", json!({"benchmark": benchmark["uuid"]})),
        (
            "update_benchmark",
            json!({"benchmark": benchmark["uuid"], "name": "renamed"}),
        ),
        ("view_measure", json!({"measure": measure["uuid"]})),
        (
            "update_measure",
            json!({"measure": measure["uuid"], "name": "renamed"}),
        ),
    ] {
        call(tool, arguments).await;
    }
    // A list result only exercises its item schema when it has items
    for tool in [
        "list_reports",
        "list_branches",
        "list_testbeds",
        "list_benchmarks",
        "list_measures",
        "list_thresholds",
        "list_plots",
        "list_jobs",
    ] {
        let list = call(tool, json!({})).await;
        assert_ne!(list["data"], json!([]), "{tool}: {list}");
    }
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
            json!({"name": "list_projects", "arguments": {}}),
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
// while authenticated tool calls are left to per-actor rate limiting
#[tokio::test]
async fn mcp_public_rate_limit() {
    use bencher_json::system::config::JsonRateLimiting;
    use bencher_schema::context::RateLimiting;

    let rate_limiting: JsonRateLimiting =
        serde_json::from_value(json!({"public": {"requests": {"minute": 1}}}))
            .expect("Invalid rate limiting");
    let server = TestServer::new_with_rate_limiting(RateLimiting::from(rate_limiting)).await;
    let user = server
        .signup("MCP Throttled", "mcp-throttled@bencher.dev")
        .await;

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
        post_from_ip(None, tools_list).await,
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
    assert_eq!(
        post_from_ip(
            Some(&user.token),
            call_tool_request("list_projects", &json!({}))
        )
        .await,
        StatusCode::OK
    );
    assert_eq!(
        post_from_ip(None, call_tool_request("list_projects", &json!({}))).await,
        StatusCode::TOO_MANY_REQUESTS
    );
    // Messages the SDK does not dispatch as a tool call are not one,
    // whatever credential they carry
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
        StatusCode::TOO_MANY_REQUESTS
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
        StatusCode::TOO_MANY_REQUESTS
    );
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
