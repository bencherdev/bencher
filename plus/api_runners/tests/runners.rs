#![expect(unused_crate_dependencies, clippy::tests_outside_test_module)]
//! Integration tests for runner CRUD endpoints.

use bencher_api_tests::TestServer;
use bencher_json::{JsonRunner, JsonRunnerToken, runner::JsonRunners};
use http::StatusCode;

// POST /v0/runners - admin can create runner
#[tokio::test]
async fn test_runners_create_as_admin() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "runneradmin@example.com").await;

    let body = serde_json::json!({
        "name": "My Test Runner"
    });

    let resp = server
        .client
        .post(server.api_url("/v0/runners"))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
    let runner_token: JsonRunnerToken = resp.json().await.expect("Failed to parse response");
    assert!(!runner_token.uuid.to_string().is_empty());
    // Token should start with prefix
    let token_str: &str = runner_token.token.as_ref();
    assert!(token_str.starts_with("bencher_runner_"));
}

// POST /v0/runners - non-admin cannot create runner
#[tokio::test]
async fn test_runners_create_forbidden_for_non_admin() {
    let server = TestServer::new().await;
    let _admin = server.signup("Admin", "adminrun1@example.com").await;
    let user = server.signup("User", "userrun1@example.com").await;

    let body = serde_json::json!({
        "name": "My Runner"
    });

    let resp = server
        .client
        .post(server.api_url("/v0/runners"))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// POST /v0/runners - create with custom slug
#[tokio::test]
async fn test_runners_create_with_slug() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "runneradmin2@example.com").await;

    let body = serde_json::json!({
        "name": "My Test Runner",
        "slug": "custom-runner-slug"
    });

    let resp = server
        .client
        .post(server.api_url("/v0/runners"))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);

    // Now get the runner by slug
    let resp = server
        .client
        .get(server.api_url("/v0/runners/custom-runner-slug"))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let runner: JsonRunner = resp.json().await.expect("Failed to parse response");
    assert_eq!(runner.name.as_ref(), "My Test Runner");
}

// GET /v0/runners - admin can list runners
#[tokio::test]
async fn test_runners_list_as_admin() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "runneradmin3@example.com").await;

    // Create a runner first
    let body = serde_json::json!({
        "name": "List Test Runner"
    });
    server
        .client
        .post(server.api_url("/v0/runners"))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    // List runners
    let resp = server
        .client
        .get(server.api_url("/v0/runners"))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let runners: JsonRunners = resp.json().await.expect("Failed to parse response");
    assert!(!runners.0.is_empty());
}

// GET /v0/runners/{runner} - get by UUID
#[tokio::test]
async fn test_runners_get_by_uuid() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "runneradmin4@example.com").await;

    // Create a runner
    let body = serde_json::json!({
        "name": "Get Test Runner"
    });
    let resp = server
        .client
        .post(server.api_url("/v0/runners"))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    let runner_token: JsonRunnerToken = resp.json().await.expect("Failed to parse response");

    // Get by UUID
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/runners/{}", runner_token.uuid)))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let runner: JsonRunner = resp.json().await.expect("Failed to parse response");
    assert_eq!(runner.uuid, runner_token.uuid);
    assert_eq!(runner.name.as_ref(), "Get Test Runner");
}

// PATCH /v0/runners/{runner} - update runner name
#[tokio::test]
async fn test_runners_update_name() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "runneradmin5@example.com").await;

    // Create a runner
    let body = serde_json::json!({
        "name": "Original Name"
    });
    let resp = server
        .client
        .post(server.api_url("/v0/runners"))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    let runner_token: JsonRunnerToken = resp.json().await.expect("Failed to parse response");

    // Update name
    let body = serde_json::json!({
        "name": "Updated Name"
    });
    let resp = server
        .client
        .patch(server.api_url(&format!("/v0/runners/{}", runner_token.uuid)))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let runner: JsonRunner = resp.json().await.expect("Failed to parse response");
    assert_eq!(runner.name.as_ref(), "Updated Name");
}

// PATCH /v0/runners/{runner} - lock runner
#[tokio::test]
async fn test_runners_lock() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "runneradmin6@example.com").await;

    // Create a runner
    let body = serde_json::json!({
        "name": "Lock Test Runner"
    });
    let resp = server
        .client
        .post(server.api_url("/v0/runners"))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    let runner_token: JsonRunnerToken = resp.json().await.expect("Failed to parse response");

    // Verify not locked
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/runners/{}", runner_token.uuid)))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");

    let runner: JsonRunner = resp.json().await.expect("Failed to parse response");
    assert!(runner.locked.is_none());

    // Lock the runner - use current timestamp
    let body = serde_json::json!({
        "locked": "2024-01-01T00:00:00Z"
    });
    let resp = server
        .client
        .patch(server.api_url(&format!("/v0/runners/{}", runner_token.uuid)))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let runner: JsonRunner = resp.json().await.expect("Failed to parse response");
    assert!(runner.locked.is_some());
}

// PATCH /v0/runners/{runner} - archive runner
#[tokio::test]
async fn test_runners_archive() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "runneradmin7@example.com").await;

    // Create a runner
    let body = serde_json::json!({
        "name": "Archive Test Runner"
    });
    let resp = server
        .client
        .post(server.api_url("/v0/runners"))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    let runner_token: JsonRunnerToken = resp.json().await.expect("Failed to parse response");

    // Archive the runner
    let body = serde_json::json!({
        "archived": "2024-01-01T00:00:00Z"
    });
    let resp = server
        .client
        .patch(server.api_url(&format!("/v0/runners/{}", runner_token.uuid)))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let runner: JsonRunner = resp.json().await.expect("Failed to parse response");
    assert!(runner.archived.is_some());

    // Archived runner should not appear in list (by default)
    let resp = server
        .client
        .get(server.api_url("/v0/runners"))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");

    let runners: JsonRunners = resp.json().await.expect("Failed to parse response");
    let found = runners.0.iter().any(|r| r.uuid == runner_token.uuid);
    assert!(!found, "Archived runner should not appear in list");
}

// GET /v0/runners?archived=true - list includes archived
#[tokio::test]
async fn test_runners_list_with_archived() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "runneradmin8@example.com").await;

    // Create and archive a runner
    let body = serde_json::json!({
        "name": "Archived Runner"
    });
    let resp = server
        .client
        .post(server.api_url("/v0/runners"))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    let runner_token: JsonRunnerToken = resp.json().await.expect("Failed to parse response");

    let body = serde_json::json!({
        "archived": "2024-01-01T00:00:00Z"
    });
    server
        .client
        .patch(server.api_url(&format!("/v0/runners/{}", runner_token.uuid)))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    // List with archived=true
    let resp = server
        .client
        .get(server.api_url("/v0/runners?archived=true"))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");

    let runners: JsonRunners = resp.json().await.expect("Failed to parse response");
    let found = runners.0.iter().any(|r| r.uuid == runner_token.uuid);
    assert!(found, "Archived runner should appear when archived=true");
}
