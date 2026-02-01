//! Integration tests for server backup endpoint.

use bencher_api_tests::TestServer;
use http::StatusCode;

// POST /v0/server/backup - requires admin auth
#[tokio::test]
async fn test_backup_requires_auth() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .post(server.api_url("/v0/server/backup"))
        .send()
        .await
        .expect("Request failed");

    // Should require authentication
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// POST /v0/server/backup - non-admin cannot backup
#[tokio::test]
async fn test_backup_forbidden_for_non_admin() {
    let server = TestServer::new().await;
    // First user is admin
    let _admin = server.signup("Admin User", "backupadmin@example.com").await;
    // Second user is NOT admin
    let user = server.signup("Regular User", "backupuser@example.com").await;

    let body = serde_json::json!({});

    let resp = server
        .client
        .post(server.api_url("/v0/server/backup"))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    // Non-admin should be forbidden
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// POST /v0/server/backup - admin can trigger backup
#[tokio::test]
async fn test_backup_as_admin() {
    let server = TestServer::new().await;
    // First user is admin
    let admin = server.signup("Admin User", "adminbackup@example.com").await;

    let body = serde_json::json!({
        "compress": false,
        "rm": false
    });

    let resp = server
        .client
        .post(server.api_url("/v0/server/backup"))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    // Admin should be able to trigger backup
    assert_eq!(resp.status(), StatusCode::CREATED);
}
