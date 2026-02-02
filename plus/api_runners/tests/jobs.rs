#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args
)]
//! Integration tests for runner agent job endpoints.
//!
//! Note: These tests require a job to exist in the database.
//! Since jobs are tied to reports, which require projects and other setup,
//! some tests may be marked as ignored until full integration is available.

use bencher_api_tests::TestServer;
use bencher_json::JsonRunnerToken;
use http::StatusCode;

// Helper to create a runner and get its token
async fn create_runner(server: &TestServer, admin_token: &str, name: &str) -> JsonRunnerToken {
    let body = serde_json::json!({
        "name": name
    });
    let resp = server
        .client
        .post(server.api_url("/v0/runners"))
        .header("Authorization", format!("Bearer {}", admin_token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    resp.json().await.expect("Failed to parse response")
}

// POST /v0/runners/{runner}/jobs - claim job with valid token (no jobs available)
#[tokio::test]
async fn test_claim_job_no_jobs() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "jobsadmin@example.com").await;

    let runner = create_runner(&server, &admin.token, "Claim Test Runner").await;
    let runner_token: &str = runner.token.as_ref();

    let body = serde_json::json!({
        "poll_timeout": 1
    });

    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
        .header("Authorization", format!("Bearer {}", runner_token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    // Should return null/empty when no jobs are available
    let body: Option<serde_json::Value> = resp.json().await.expect("Failed to parse response");
    assert!(body.is_none());
}

// POST /v0/runners/{runner}/jobs - invalid token rejected
#[tokio::test]
async fn test_claim_job_invalid_token() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "jobsadmin2@example.com").await;

    let runner = create_runner(&server, &admin.token, "Invalid Token Runner").await;

    let body = serde_json::json!({
        "poll_timeout": 1
    });

    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
        .header("Authorization", "Bearer bencher_runner_invalid_token")
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// POST /v0/runners/{runner}/jobs - token for wrong runner rejected
#[tokio::test]
async fn test_claim_job_wrong_runner_token() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "jobsadmin3@example.com").await;

    let runner1 = create_runner(&server, &admin.token, "Runner One").await;
    let runner2 = create_runner(&server, &admin.token, "Runner Two").await;
    let runner1_token: &str = runner1.token.as_ref();

    let body = serde_json::json!({
        "poll_timeout": 1
    });

    // Try to claim job for runner2 using runner1's token
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/jobs", runner2.uuid)))
        .header("Authorization", format!("Bearer {}", runner1_token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// POST /v0/runners/{runner}/jobs - locked runner rejected
#[tokio::test]
async fn test_claim_job_locked_runner() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "jobsadmin4@example.com").await;

    let runner = create_runner(&server, &admin.token, "Locked Runner").await;
    let runner_token: &str = runner.token.as_ref();

    // Lock the runner
    let body = serde_json::json!({
        "locked": "2024-01-01T00:00:00Z"
    });
    server
        .client
        .patch(server.api_url(&format!("/v0/runners/{}", runner.uuid)))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    // Try to claim job with locked runner
    let body = serde_json::json!({
        "poll_timeout": 1
    });
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
        .header("Authorization", format!("Bearer {}", runner_token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// POST /v0/runners/{runner}/jobs - missing Authorization header
#[tokio::test]
async fn test_claim_job_missing_auth() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "jobsadmin5@example.com").await;

    let runner = create_runner(&server, &admin.token, "No Auth Runner").await;

    let body = serde_json::json!({
        "poll_timeout": 1
    });

    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// PATCH /v0/runners/{runner}/jobs/{job} - invalid token rejected
#[tokio::test]
async fn test_update_job_invalid_token() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "jobsadmin6@example.com").await;

    let runner = create_runner(&server, &admin.token, "Update Invalid Token Runner").await;

    // Use a fake job UUID
    let fake_job_uuid = "00000000-0000-0000-0000-000000000000";

    let body = serde_json::json!({
        "status": "running"
    });

    let resp = server
        .client
        .patch(server.api_url(&format!(
            "/v0/runners/{}/jobs/{}",
            runner.uuid, fake_job_uuid
        )))
        .header("Authorization", "Bearer bencher_runner_invalid")
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// PATCH /v0/runners/{runner}/jobs/{job} - job not found
#[tokio::test]
async fn test_update_job_not_found() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "jobsadmin7@example.com").await;

    let runner = create_runner(&server, &admin.token, "Job Not Found Runner").await;
    let runner_token: &str = runner.token.as_ref();

    // Use a fake job UUID
    let fake_job_uuid = "00000000-0000-0000-0000-000000000000";

    let body = serde_json::json!({
        "status": "running"
    });

    let resp = server
        .client
        .patch(server.api_url(&format!(
            "/v0/runners/{}/jobs/{}",
            runner.uuid, fake_job_uuid
        )))
        .header("Authorization", format!("Bearer {}", runner_token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// POST /v0/runners/{runner}/jobs - token without prefix rejected
#[tokio::test]
async fn test_claim_job_no_prefix_token() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "jobsadmin8@example.com").await;

    let runner = create_runner(&server, &admin.token, "No Prefix Runner").await;

    let body = serde_json::json!({
        "poll_timeout": 1
    });

    // Use a token without the bencher_runner_ prefix
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
        .header("Authorization", "Bearer some_random_token_without_prefix")
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// POST /v0/runners/{runner}/jobs - archived runner rejected
#[tokio::test]
async fn test_claim_job_archived_runner() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "jobsadmin9@example.com").await;

    let runner = create_runner(&server, &admin.token, "Archived Runner").await;
    let runner_token: &str = runner.token.as_ref();

    // Archive the runner
    let body = serde_json::json!({
        "archived": "2024-01-01T00:00:00Z"
    });
    server
        .client
        .patch(server.api_url(&format!("/v0/runners/{}", runner.uuid)))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    // Try to claim job with archived runner
    let body = serde_json::json!({
        "poll_timeout": 1
    });
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
        .header("Authorization", format!("Bearer {}", runner_token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}
