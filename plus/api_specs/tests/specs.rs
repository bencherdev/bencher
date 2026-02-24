#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::decimal_literal_representation,
    clippy::indexing_slicing
)]
//! Integration tests for spec CRUD endpoints.

use bencher_api_tests::TestServer;
use bencher_json::{JsonRunnerToken, JsonSpec, JsonSpecs};
use http::StatusCode;

// POST /v0/specs - admin can create spec
#[tokio::test]
async fn specs_create() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "specadmin@example.com").await;

    let body = serde_json::json!({
        "name": "Small x86",
        "architecture": "x86_64",
        "cpu": 2,
        "memory": 4_294_967_296i64,
        "disk": 10_737_418_240i64,
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
    assert_eq!(value["name"], "Small x86");
    assert_eq!(value["slug"], "small-x86");
    assert_eq!(value["cpu"], 2);
    assert_eq!(value["memory"], 4_294_967_296i64);
    assert_eq!(value["disk"], 10_737_418_240i64);
    assert_eq!(value["network"], false);
    assert!(value["archived"].is_null());
}

// POST /v0/specs - custom slug on create
#[tokio::test]
async fn specs_create_custom_slug() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "specslug@example.com").await;

    let body = serde_json::json!({
        "name": "My Custom Spec",
        "slug": "my-custom-slug",
        "architecture": "x86_64",
        "cpu": 2,
        "memory": 4_294_967_296i64,
        "disk": 10_737_418_240i64,
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
    assert_eq!(AsRef::<str>::as_ref(&spec.slug), "my-custom-slug");
}

// POST /v0/specs - non-admin cannot create spec
#[tokio::test]
async fn specs_create_forbidden_for_non_admin() {
    let server = TestServer::new().await;
    let _admin = server.signup("Admin", "specadmin2@example.com").await;
    let user = server.signup("User", "specuser2@example.com").await;

    let body = serde_json::json!({
        "name": "Forbidden Spec",
        "architecture": "x86_64",
        "cpu": 2,
        "memory": 4_294_967_296i64,
        "disk": 10_737_418_240i64,
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
async fn specs_list() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "specadmin3@example.com").await;

    // Create a spec first
    let body = serde_json::json!({
        "name": "List Spec",
        "architecture": "x86_64",
        "cpu": 4,
        "memory": 8_589_934_592i64,
        "disk": 21_474_836_480i64,
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
async fn specs_get_by_uuid() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "specadmin4@example.com").await;

    // Create a spec
    let body = serde_json::json!({
        "name": "Get By UUID Spec",
        "architecture": "x86_64",
        "cpu": 2,
        "memory": 4_294_967_296i64,
        "disk": 10_737_418_240i64,
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

// GET /v0/specs/{slug} - get by slug
#[tokio::test]
async fn specs_get_by_slug() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "specslugget@example.com").await;

    // Create a spec
    let body = serde_json::json!({
        "name": "Slug Lookup Spec",
        "architecture": "x86_64",
        "cpu": 2,
        "memory": 4_294_967_296i64,
        "disk": 10_737_418_240i64,
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

    // Get by slug
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/specs/{}", created.slug)))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let spec: JsonSpec = resp.json().await.expect("Failed to parse response");
    assert_eq!(spec.uuid, created.uuid);
    assert_eq!(spec.slug, created.slug);
}

// PATCH /v0/specs/{uuid} - archive spec
#[tokio::test]
async fn specs_archive() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "specadmin5@example.com").await;

    // Create a spec
    let body = serde_json::json!({
        "name": "Archive Spec",
        "architecture": "x86_64",
        "cpu": 2,
        "memory": 4_294_967_296i64,
        "disk": 10_737_418_240i64,
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
async fn specs_list_with_archived() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "specadmin6@example.com").await;

    // Create and archive a spec
    let body = serde_json::json!({
        "name": "Archived List Spec",
        "architecture": "x86_64",
        "cpu": 2,
        "memory": 4_294_967_296i64,
        "disk": 10_737_418_240i64,
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
async fn specs_unarchive() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "specadmin7@example.com").await;

    // Create and archive a spec
    let body = serde_json::json!({
        "name": "Unarchive Spec",
        "architecture": "x86_64",
        "cpu": 2,
        "memory": 4_294_967_296i64,
        "disk": 10_737_418_240i64,
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
async fn specs_delete() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "specadmin8@example.com").await;

    // Create a spec
    let body = serde_json::json!({
        "name": "Delete Spec",
        "architecture": "x86_64",
        "cpu": 2,
        "memory": 4_294_967_296i64,
        "disk": 10_737_418_240i64,
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
async fn specs_delete_fails_when_in_use() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "specfk@example.com").await;

    // Create a spec
    let body = serde_json::json!({
        "name": "FK Test Spec",
        "architecture": "x86_64",
        "cpu": 2,
        "memory": 4_294_967_296i64,
        "disk": 10_737_418_240i64,
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

// POST /v0/specs - create with fallback: true
#[tokio::test]
async fn specs_create_fallback() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "specfb1@example.com").await;

    let body = serde_json::json!({
        "name": "Fallback Spec",
        "architecture": "x86_64",
        "cpu": 2,
        "memory": 4_294_967_296i64,
        "disk": 10_737_418_240i64,
        "fallback": true
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
    assert!(spec.fallback.is_some(), "fallback should be set");
}

// POST /v0/specs - create without fallback field
#[tokio::test]
async fn specs_create_without_fallback() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "specfb2@example.com").await;

    let body = serde_json::json!({
        "name": "No Fallback Spec",
        "architecture": "x86_64",
        "cpu": 2,
        "memory": 4_294_967_296i64,
        "disk": 10_737_418_240i64
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
    assert!(spec.fallback.is_none(), "fallback should not be set");
}

// POST /v0/specs - create with fallback: false
#[tokio::test]
async fn specs_create_fallback_false() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "specfb3@example.com").await;

    let body = serde_json::json!({
        "name": "Fallback False Spec",
        "architecture": "x86_64",
        "cpu": 2,
        "memory": 4_294_967_296i64,
        "disk": 10_737_418_240i64,
        "fallback": false
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
    assert!(
        spec.fallback.is_none(),
        "fallback should not be set when false"
    );
}

// POST /v0/specs - creating a new fallback replaces the existing one
#[tokio::test]
async fn specs_create_fallback_replaces_existing() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "specfb4@example.com").await;

    // Create spec A with fallback
    let body_a = serde_json::json!({
        "name": "Fallback A",
        "architecture": "x86_64",
        "cpu": 2,
        "memory": 4_294_967_296i64,
        "disk": 10_737_418_240i64,
        "fallback": true
    });
    let resp = server
        .client
        .post(server.api_url("/v0/specs"))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body_a)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let spec_a: JsonSpec = resp.json().await.expect("Failed to parse response");
    assert!(spec_a.fallback.is_some());

    // Create spec B with fallback
    let body_b = serde_json::json!({
        "name": "Fallback B",
        "architecture": "x86_64",
        "cpu": 4,
        "memory": 8_589_934_592i64,
        "disk": 21_474_836_480i64,
        "fallback": true
    });
    let resp = server
        .client
        .post(server.api_url("/v0/specs"))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body_b)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let spec_b: JsonSpec = resp.json().await.expect("Failed to parse response");
    assert!(spec_b.fallback.is_some(), "B should be fallback");

    // Verify A is no longer fallback
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/specs/{}", spec_a.uuid)))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");
    let spec_a_updated: JsonSpec = resp.json().await.expect("Failed to parse response");
    assert!(
        spec_a_updated.fallback.is_none(),
        "A should no longer be fallback"
    );
}

// PATCH /v0/specs/{uuid} - set fallback via update
#[tokio::test]
async fn specs_update_set_fallback() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "specfb5@example.com").await;

    // Create spec without fallback
    let body = serde_json::json!({
        "name": "Set Fallback Spec",
        "architecture": "x86_64",
        "cpu": 2,
        "memory": 4_294_967_296i64,
        "disk": 10_737_418_240i64
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
    assert!(created.fallback.is_none());

    // Set fallback via PATCH
    let body = serde_json::json!({"fallback": true});
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
    assert!(spec.fallback.is_some(), "fallback should now be set");
}

// PATCH /v0/specs/{uuid} - unset fallback via update
#[tokio::test]
async fn specs_update_unset_fallback() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "specfb6@example.com").await;

    // Create spec with fallback
    let body = serde_json::json!({
        "name": "Unset Fallback Spec",
        "architecture": "x86_64",
        "cpu": 2,
        "memory": 4_294_967_296i64,
        "disk": 10_737_418_240i64,
        "fallback": true
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
    assert!(created.fallback.is_some());

    // Unset fallback via PATCH
    let body = serde_json::json!({"fallback": false});
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
    assert!(spec.fallback.is_none(), "fallback should be unset");
}

// PATCH /v0/specs/{uuid} - fallback unchanged when not included in update
#[tokio::test]
async fn specs_update_fallback_no_change() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "specfb7@example.com").await;

    // Create spec with fallback
    let body = serde_json::json!({
        "name": "No Change Fallback",
        "architecture": "x86_64",
        "cpu": 2,
        "memory": 4_294_967_296i64,
        "disk": 10_737_418_240i64,
        "fallback": true
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
    assert!(created.fallback.is_some());

    // Update only name, not fallback
    let body = serde_json::json!({"name": "Renamed Fallback"});
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
    assert!(spec.fallback.is_some(), "fallback should still be set");
}

// PATCH /v0/specs/{uuid} - setting fallback on one spec clears it from another
#[tokio::test]
async fn specs_update_fallback_replaces_existing() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "specfb8@example.com").await;

    // Create spec A with fallback
    let body_a = serde_json::json!({
        "name": "Update Replace A",
        "architecture": "x86_64",
        "cpu": 2,
        "memory": 4_294_967_296i64,
        "disk": 10_737_418_240i64,
        "fallback": true
    });
    let resp = server
        .client
        .post(server.api_url("/v0/specs"))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body_a)
        .send()
        .await
        .expect("Request failed");
    let spec_a: JsonSpec = resp.json().await.expect("Failed to parse response");

    // Create spec B without fallback
    let body_b = serde_json::json!({
        "name": "Update Replace B",
        "architecture": "x86_64",
        "cpu": 4,
        "memory": 8_589_934_592i64,
        "disk": 21_474_836_480i64
    });
    let resp = server
        .client
        .post(server.api_url("/v0/specs"))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body_b)
        .send()
        .await
        .expect("Request failed");
    let spec_b: JsonSpec = resp.json().await.expect("Failed to parse response");

    // Set B as fallback via PATCH
    let body = serde_json::json!({"fallback": true});
    let resp = server
        .client
        .patch(server.api_url(&format!("/v0/specs/{}", spec_b.uuid)))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let spec_b_updated: JsonSpec = resp.json().await.expect("Failed to parse response");
    assert!(spec_b_updated.fallback.is_some(), "B should be fallback");

    // Verify A is no longer fallback
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/specs/{}", spec_a.uuid)))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");
    let spec_a_refreshed: JsonSpec = resp.json().await.expect("Failed to parse response");
    assert!(
        spec_a_refreshed.fallback.is_none(),
        "A should no longer be fallback"
    );
}

// PATCH /v0/specs/{uuid} - archiving clears fallback
#[tokio::test]
async fn specs_archive_clears_fallback() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "specfb9@example.com").await;

    // Create spec with fallback
    let body = serde_json::json!({
        "name": "Archive Fallback",
        "architecture": "x86_64",
        "cpu": 2,
        "memory": 4_294_967_296i64,
        "disk": 10_737_418_240i64,
        "fallback": true
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
    assert!(created.fallback.is_some());

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
    assert!(spec.archived.is_some(), "spec should be archived");
    assert!(
        spec.fallback.is_none(),
        "fallback should be cleared after archiving"
    );
}

// DELETE + re-create - deleting fallback spec does not auto-promote
#[tokio::test]
async fn specs_delete_fallback_spec() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "specfb10@example.com").await;

    // Create spec with fallback
    let body = serde_json::json!({
        "name": "Delete Fallback",
        "architecture": "x86_64",
        "cpu": 2,
        "memory": 4_294_967_296i64,
        "disk": 10_737_418_240i64,
        "fallback": true
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

    // Delete the fallback spec
    let resp = server
        .client
        .delete(server.api_url(&format!("/v0/specs/{}", created.uuid)))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Create a new spec without fallback
    let body = serde_json::json!({
        "name": "After Delete Spec",
        "architecture": "x86_64",
        "cpu": 2,
        "memory": 4_294_967_296i64,
        "disk": 10_737_418_240i64
    });
    let resp = server
        .client
        .post(server.api_url("/v0/specs"))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    let new_spec: JsonSpec = resp.json().await.expect("Failed to parse response");
    assert!(
        new_spec.fallback.is_none(),
        "new spec should not be auto-promoted to fallback"
    );
}

// GET /v0/specs - list includes fallback field
#[tokio::test]
async fn specs_list_includes_fallback_field() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "specfb11@example.com").await;

    // Create spec with fallback
    let body = serde_json::json!({
        "name": "List Fallback Yes",
        "architecture": "x86_64",
        "cpu": 2,
        "memory": 4_294_967_296i64,
        "disk": 10_737_418_240i64,
        "fallback": true
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
    let fb_spec: JsonSpec = resp.json().await.expect("Failed to parse response");

    // Create spec without fallback
    let body = serde_json::json!({
        "name": "List Fallback No",
        "architecture": "x86_64",
        "cpu": 4,
        "memory": 8_589_934_592i64,
        "disk": 21_474_836_480i64
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
    let no_fb_spec: JsonSpec = resp.json().await.expect("Failed to parse response");

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

    let fb = specs
        .0
        .iter()
        .find(|s| s.uuid == fb_spec.uuid)
        .expect("fallback spec not found");
    assert!(
        fb.fallback.is_some(),
        "fallback spec should have fallback set in list"
    );

    let no_fb = specs
        .0
        .iter()
        .find(|s| s.uuid == no_fb_spec.uuid)
        .expect("non-fallback spec not found");
    assert!(
        no_fb.fallback.is_none(),
        "non-fallback spec should have fallback unset in list"
    );
}

// POST /v0/specs - creating a new fallback replaces an archived fallback
#[tokio::test]
async fn specs_create_fallback_replaces_archived_fallback() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "specfb12@example.com").await;

    // Create spec A with fallback
    let body_a = serde_json::json!({
        "name": "Archived FB A",
        "architecture": "x86_64",
        "cpu": 2,
        "memory": 4_294_967_296i64,
        "disk": 10_737_418_240i64,
        "fallback": true
    });
    let resp = server
        .client
        .post(server.api_url("/v0/specs"))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body_a)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let spec_a: JsonSpec = resp.json().await.expect("Failed to parse response");
    assert!(spec_a.fallback.is_some());

    // Archive A — fallback should be cleared
    let body = serde_json::json!({"archived": true});
    let resp = server
        .client
        .patch(server.api_url(&format!("/v0/specs/{}", spec_a.uuid)))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let spec_a_archived: JsonSpec = resp.json().await.expect("Failed to parse response");
    assert!(spec_a_archived.archived.is_some());
    assert!(
        spec_a_archived.fallback.is_none(),
        "A's fallback should be cleared after archiving"
    );

    // Create spec B with fallback
    let body_b = serde_json::json!({
        "name": "Archived FB B",
        "architecture": "x86_64",
        "cpu": 4,
        "memory": 8_589_934_592i64,
        "disk": 21_474_836_480i64,
        "fallback": true
    });
    let resp = server
        .client
        .post(server.api_url("/v0/specs"))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body_b)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let spec_b: JsonSpec = resp.json().await.expect("Failed to parse response");
    assert!(spec_b.fallback.is_some(), "B should be fallback");

    // Re-fetch A, verify it still has no fallback
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/specs/{}", spec_a.uuid)))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");
    let spec_a_refetched: JsonSpec = resp.json().await.expect("Failed to parse response");
    assert!(
        spec_a_refetched.fallback.is_none(),
        "A should still have no fallback"
    );
}

// PATCH /v0/specs/{uuid} - setting fallback on a spec that already is fallback (idempotent)
#[tokio::test]
async fn specs_update_set_fallback_already_fallback() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "specfb13@example.com").await;

    // Create spec with fallback
    let body = serde_json::json!({
        "name": "Idempotent FB",
        "architecture": "x86_64",
        "cpu": 2,
        "memory": 4_294_967_296i64,
        "disk": 10_737_418_240i64,
        "fallback": true
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
    let created: JsonSpec = resp.json().await.expect("Failed to parse response");
    assert!(created.fallback.is_some());

    // Set fallback again (idempotent)
    let body = serde_json::json!({"fallback": true});
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
    assert!(
        spec.fallback.is_some(),
        "fallback should still be set (idempotent)"
    );
}

// PATCH /v0/specs/{uuid} - unsetting fallback on a spec that is not fallback (no-op)
#[tokio::test]
async fn specs_update_unset_fallback_already_unset() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "specfb14@example.com").await;

    // Create spec without fallback
    let body = serde_json::json!({
        "name": "Noop Unset FB",
        "architecture": "x86_64",
        "cpu": 2,
        "memory": 4_294_967_296i64,
        "disk": 10_737_418_240i64
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
    let created: JsonSpec = resp.json().await.expect("Failed to parse response");
    assert!(created.fallback.is_none());

    // Unset fallback (no-op)
    let body = serde_json::json!({"fallback": false});
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
    assert!(
        spec.fallback.is_none(),
        "fallback should still be unset (no-op)"
    );
}
