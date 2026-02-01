//! Integration tests for server OpenAPI spec endpoint.

use bencher_api_tests::TestServer;
use bencher_json::JsonSpec;
use http::StatusCode;

// GET /v0/server/spec - public, no auth required
#[tokio::test]
async fn test_spec_get() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .get(server.api_url("/v0/server/spec"))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let body: JsonSpec = resp.json().await.expect("Failed to parse response");
    assert!(body.version().is_some());
}

// Verify spec contains expected paths
#[tokio::test]
async fn test_spec_contains_paths() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .get(server.api_url("/v0/server/spec"))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.expect("Failed to parse response");

    // Verify it's a valid OpenAPI spec with paths
    assert!(body.get("paths").is_some(), "Spec should have paths");
    assert!(body.get("info").is_some(), "Spec should have info");
}

// Verify spec has correct OpenAPI version
#[tokio::test]
async fn test_spec_openapi_version() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .get(server.api_url("/v0/server/spec"))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.expect("Failed to parse response");

    // Should be OpenAPI 3.x
    let openapi = body.get("openapi").and_then(|v| v.as_str());
    assert!(openapi.is_some_and(|v| v.starts_with("3.")), "Should be OpenAPI 3.x");
}
