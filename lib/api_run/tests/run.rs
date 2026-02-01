//! Integration tests for run endpoint.

use bencher_api_tests::TestServer;
use bencher_json::JsonReport;
use http::StatusCode;

// POST /v0/run - create a run with authentication
#[tokio::test]
async fn test_run_post_authenticated() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "runauth@example.com").await;
    let org = server.create_org(&user, "Run Org").await;
    let project = server.create_project(&user, &org, "Run Project").await;

    let project_slug: &str = project.slug.as_ref();
    // BMF format results
    let bmf_results = serde_json::json!({
        "benchmark_name": {
            "latency": {
                "value": 100.0
            }
        }
    });

    let body = serde_json::json!({
        "project": project_slug,
        "branch": "main",
        "testbed": "localhost",
        "start_time": "2024-01-01T00:00:00Z",
        "end_time": "2024-01-01T00:01:00Z",
        "results": [bmf_results.to_string()]
    });

    let resp = server
        .client
        .post(server.api_url("/v0/run"))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
    let _report: JsonReport = resp.json().await.expect("Failed to parse response");
}

// POST /v0/run - run with existing project creates branch/testbed as needed
#[tokio::test]
async fn test_run_post_creates_branch_testbed() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "runcreate@example.com").await;
    let org = server.create_org(&user, "Run Create Org").await;
    let project = server.create_project(&user, &org, "Auto Create Project").await;

    // BMF format results
    let bmf_results = serde_json::json!({
        "benchmark_name": {
            "latency": {
                "value": 100.0
            }
        }
    });

    let project_slug: &str = project.slug.as_ref();
    // Run with new branch and testbed names that don't exist yet
    let body = serde_json::json!({
        "project": project_slug,
        "branch": "feature-branch",
        "testbed": "new-testbed",
        "start_time": "2024-01-01T00:00:00Z",
        "end_time": "2024-01-01T00:01:00Z",
        "results": [bmf_results.to_string()]
    });

    let resp = server
        .client
        .post(server.api_url("/v0/run"))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    // Should create the branch, testbed, and run successfully
    assert_eq!(resp.status(), StatusCode::CREATED);
}

// POST /v0/run - run without authentication (public run)
#[tokio::test]
async fn test_run_post_unauthenticated() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "runpublic@example.com").await;
    let org = server.create_org(&user, "Public Run Org").await;
    let project = server.create_project(&user, &org, "Public Run Project").await;

    // BMF format results
    let bmf_results = serde_json::json!({
        "benchmark_name": {
            "latency": {
                "value": 100.0
            }
        }
    });

    let project_slug: &str = project.slug.as_ref();
    let body = serde_json::json!({
        "project": project_slug,
        "branch": "main",
        "testbed": "localhost",
        "start_time": "2024-01-01T00:00:00Z",
        "end_time": "2024-01-01T00:01:00Z",
        "results": [bmf_results.to_string()]
    });

    // Try without authentication - should fail for non-public project
    let resp = server
        .client
        .post(server.api_url("/v0/run"))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    // Without auth, should get unauthorized or forbidden
    assert!(
        resp.status() == StatusCode::UNAUTHORIZED || resp.status() == StatusCode::FORBIDDEN,
        "Expected auth error, got: {}",
        resp.status()
    );
}
