#![expect(unused_crate_dependencies, clippy::tests_outside_test_module)]
//! Integration tests for spec CRUD endpoints.

use bencher_api_tests::TestServer;
use bencher_json::{JsonRunnerToken, JsonSpec, JsonSpecs};
use http::StatusCode;

// POST /v0/specs - admin can create spec
#[tokio::test]
async fn test_specs_create() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "specadmin@example.com").await;

    let body = serde_json::json!({
        "cpu": 2,
        "memory": 4_294_967_296_i64,
        "disk": 10_737_418_240_i64,
        "network": false
    });

    let resp = server
        .client
        .post(server.api_url("/v0/specs"))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
    let spec: JsonSpec = resp.json().await.expect("Failed to parse response");
    // Verify fields round-trip correctly
    let value = serde_json::to_value(&spec).expect("Failed to serialize");
    assert_eq!(value["cpu"], 2);
    assert_eq!(value["memory"], 4_294_967_296_i64);
    assert_eq!(value["disk"], 10_737_418_240_i64);
    assert_eq!(value["network"], false);
    assert!(value["archived"].is_null());
}

// POST /v0/specs - non-admin cannot create spec
#[tokio::test]
async fn test_specs_create_forbidden_for_non_admin() {
    let server = TestServer::new().await;
    let _admin = server.signup("Admin", "specadmin2@example.com").await;
    let user = server.signup("User", "specuser2@example.com").await;

    let body = serde_json::json!({
        "cpu": 2,
        "memory": 4_294_967_296_i64,
        "disk": 10_737_418_240_i64,
        "network": false
    });

    let resp = server
        .client
        .post(server.api_url("/v0/specs"))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// GET /v0/specs - list specs
#[tokio::test]
async fn test_specs_list() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "specadmin3@example.com").await;

    // Create a spec first
    let body = serde_json::json!({
        "cpu": 4,
        "memory": 8_589_934_592_i64,
        "disk": 21_474_836_480_i64,
        "network": true
    });
    let resp = server
        .client
        .post(server.api_url("/v0/specs"))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    // List specs
    let resp = server
        .client
        .get(server.api_url("/v0/specs"))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let specs: JsonSpecs = resp.json().await.expect("Failed to parse response");
    assert!(!specs.0.is_empty());
}

// GET /v0/specs/{uuid} - get by UUID
#[tokio::test]
async fn test_specs_get_by_uuid() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "specadmin4@example.com").await;

    // Create a spec
    let body = serde_json::json!({
        "cpu": 2,
        "memory": 4_294_967_296_i64,
        "disk": 10_737_418_240_i64,
        "network": false
    });
    let resp = server
        .client
        .post(server.api_url("/v0/specs"))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    let created: JsonSpec = resp.json().await.expect("Failed to parse response");

    // Get by UUID
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/specs/{}", created.uuid)))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let spec: JsonSpec = resp.json().await.expect("Failed to parse response");
    assert_eq!(spec.uuid, created.uuid);
}

// PATCH /v0/specs/{uuid} - archive spec
#[tokio::test]
async fn test_specs_archive() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "specadmin5@example.com").await;

    // Create a spec
    let body = serde_json::json!({
        "cpu": 2,
        "memory": 4_294_967_296_i64,
        "disk": 10_737_418_240_i64,
        "network": false
    });
    let resp = server
        .client
        .post(server.api_url("/v0/specs"))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    let created: JsonSpec = resp.json().await.expect("Failed to parse response");

    // Archive the spec
    let body = serde_json::json!({"archived": true});
    let resp = server
        .client
        .patch(server.api_url(&format!("/v0/specs/{}", created.uuid)))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let spec: JsonSpec = resp.json().await.expect("Failed to parse response");
    assert!(spec.archived.is_some());

    // Archived spec should not appear in default list
    let resp = server
        .client
        .get(server.api_url("/v0/specs"))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");

    let specs: JsonSpecs = resp.json().await.expect("Failed to parse response");
    let found = specs.0.iter().any(|s| s.uuid == created.uuid);
    assert!(!found, "Archived spec should not appear in default list");
}

// GET /v0/specs?archived=true - list includes archived
#[tokio::test]
async fn test_specs_list_with_archived() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "specadmin6@example.com").await;

    // Create and archive a spec
    let body = serde_json::json!({
        "cpu": 2,
        "memory": 4_294_967_296_i64,
        "disk": 10_737_418_240_i64,
        "network": false
    });
    let resp = server
        .client
        .post(server.api_url("/v0/specs"))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    let created: JsonSpec = resp.json().await.expect("Failed to parse response");

    let body = serde_json::json!({"archived": true});
    server
        .client
        .patch(server.api_url(&format!("/v0/specs/{}", created.uuid)))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    // List with archived=true
    let resp = server
        .client
        .get(server.api_url("/v0/specs?archived=true"))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");

    let specs: JsonSpecs = resp.json().await.expect("Failed to parse response");
    let found = specs.0.iter().any(|s| s.uuid == created.uuid);
    assert!(found, "Archived spec should appear when archived=true");
}

// PATCH /v0/specs/{uuid} - unarchive spec
#[tokio::test]
async fn test_specs_unarchive() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "specadmin7@example.com").await;

    // Create and archive a spec
    let body = serde_json::json!({
        "cpu": 2,
        "memory": 4_294_967_296_i64,
        "disk": 10_737_418_240_i64,
        "network": false
    });
    let resp = server
        .client
        .post(server.api_url("/v0/specs"))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    let created: JsonSpec = resp.json().await.expect("Failed to parse response");

    let body = serde_json::json!({"archived": true});
    server
        .client
        .patch(server.api_url(&format!("/v0/specs/{}", created.uuid)))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    // Unarchive
    let body = serde_json::json!({"archived": false});
    let resp = server
        .client
        .patch(server.api_url(&format!("/v0/specs/{}", created.uuid)))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let spec: JsonSpec = resp.json().await.expect("Failed to parse response");
    assert!(spec.archived.is_none());
}

// DELETE /v0/specs/{uuid} - delete spec
#[tokio::test]
async fn test_specs_delete() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "specadmin8@example.com").await;

    // Create a spec
    let body = serde_json::json!({
        "cpu": 2,
        "memory": 4_294_967_296_i64,
        "disk": 10_737_418_240_i64,
        "network": false
    });
    let resp = server
        .client
        .post(server.api_url("/v0/specs"))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    let created: JsonSpec = resp.json().await.expect("Failed to parse response");

    // Delete spec
    let resp = server
        .client
        .delete(server.api_url(&format!("/v0/specs/{}", created.uuid)))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Verify it's gone
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/specs/{}", created.uuid)))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// DELETE /v0/specs/{uuid} - delete fails when spec is referenced by a runner
#[tokio::test]
async fn test_specs_delete_fails_when_in_use() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "specfk@example.com").await;

    // Create a spec
    let body = serde_json::json!({
        "cpu": 2,
        "memory": 4_294_967_296_i64,
        "disk": 10_737_418_240_i64,
        "network": false
    });
    let resp = server
        .client
        .post(server.api_url("/v0/specs"))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let spec: JsonSpec = resp.json().await.expect("Failed to parse response");

    // Create a runner and associate the spec with it
    let runner_body = serde_json::json!({"name": "FK Test Runner"});
    let resp = server
        .client
        .post(server.api_url("/v0/runners"))
        .header("Authorization", server.bearer(&admin.token))
        .json(&runner_body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let runner: JsonRunnerToken = resp.json().await.expect("Failed to parse response");

    // Associate spec with runner via API
    let assoc_body = serde_json::json!({"spec": spec.uuid.to_string()});
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/specs", runner.uuid)))
        .header("Authorization", server.bearer(&admin.token))
        .json(&assoc_body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Try to delete spec - should fail due to FK constraint
    let resp = server
        .client
        .delete(server.api_url(&format!("/v0/specs/{}", spec.uuid)))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CONFLICT);
}
