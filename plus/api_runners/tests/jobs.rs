#![expect(unused_crate_dependencies, clippy::tests_outside_test_module)]
//! Integration tests for runner agent job endpoints.
//!
//! Note: These tests require a job to exist in the database.
//! Since jobs are tied to reports, which require projects and other setup,
//! some tests may be marked as ignored until full integration is available.

mod common;

use bencher_api_tests::TestServer;
use bencher_json::{JsonJob, JsonUpdateJobResponse};
use common::{create_runner, create_test_report, get_project_id, insert_test_job};
use http::StatusCode;

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
        .header("Authorization", format!("Bearer {runner_token}"))
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
        .header("Authorization", format!("Bearer {runner1_token}"))
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
        .header("Authorization", format!("Bearer {runner_token}"))
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
        .patch(server.api_url(&format!("/v0/runners/{}/jobs/{fake_job_uuid}", runner.uuid)))
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
        .patch(server.api_url(&format!("/v0/runners/{}/jobs/{fake_job_uuid}", runner.uuid)))
        .header("Authorization", format!("Bearer {runner_token}"))
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
        .header("Authorization", format!("Bearer {runner_token}"))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// =============================================================================
// Job State Transition Tests
// =============================================================================
//
// These tests verify the complete job lifecycle: pending → claimed → running → completed/failed

mod job_lifecycle {
    use super::*;
    use bencher_json::JobStatus;

    // Test the full job lifecycle: claim → running → completed
    #[tokio::test]
    async fn test_job_lifecycle_completed() {
        let server = TestServer::new().await;
        let admin = server.signup("Admin", "lifecycle1@example.com").await;
        let org = server.create_org(&admin, "Lifecycle Org").await;
        let project = server
            .create_project(&admin, &org, "Lifecycle Project")
            .await;

        // Create a runner
        let runner = create_runner(&server, &admin.token, "Lifecycle Runner").await;
        let runner_token: &str = runner.token.as_ref();

        // Create test infrastructure and a pending job
        let project_id = get_project_id(&server, project.slug.as_ref());
        let report_id = create_test_report(&server, project_id);
        let job_uuid = insert_test_job(&server, report_id);

        // Step 1: Claim the job
        let body = serde_json::json!({
            "poll_timeout": 5
        });
        let resp = server
            .client
            .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        assert_eq!(resp.status(), StatusCode::OK);
        let claimed_job: Option<JsonJob> = resp.json().await.expect("Failed to parse response");
        let claimed_job = claimed_job.expect("Expected to claim a job");
        assert_eq!(claimed_job.uuid, job_uuid);
        assert_eq!(claimed_job.status, JobStatus::Claimed);

        // Step 2: Update to running
        let body = serde_json::json!({
            "status": "running"
        });
        let resp = server
            .client
            .patch(server.api_url(&format!("/v0/runners/{}/jobs/{}", runner.uuid, job_uuid)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        assert_eq!(resp.status(), StatusCode::OK);
        let response: JsonUpdateJobResponse = resp.json().await.expect("Failed to parse response");
        assert!(!response.canceled);

        // Step 3: Update to completed
        let body = serde_json::json!({
            "status": "completed",
            "exit_code": 0
        });
        let resp = server
            .client
            .patch(server.api_url(&format!("/v0/runners/{}/jobs/{}", runner.uuid, job_uuid)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        assert_eq!(resp.status(), StatusCode::OK);
        let response: JsonUpdateJobResponse = resp.json().await.expect("Failed to parse response");
        assert!(!response.canceled);

        // Verify final state via project jobs endpoint
        let project_slug: &str = project.slug.as_ref();
        let resp = server
            .client
            .get(server.api_url(&format!("/v0/projects/{project_slug}/jobs/{job_uuid}")))
            .header("Authorization", server.bearer(&admin.token))
            .send()
            .await
            .expect("Request failed");

        assert_eq!(resp.status(), StatusCode::OK);
        let final_job: JsonJob = resp.json().await.expect("Failed to parse response");
        assert_eq!(final_job.status, JobStatus::Completed);
        assert_eq!(final_job.exit_code, Some(0));
    }

    // Test the job lifecycle with failure: claim → running → failed
    #[tokio::test]
    async fn test_job_lifecycle_failed() {
        let server = TestServer::new().await;
        let admin = server.signup("Admin", "lifecycle2@example.com").await;
        let org = server.create_org(&admin, "Lifecycle Fail Org").await;
        let project = server
            .create_project(&admin, &org, "Lifecycle Fail Project")
            .await;

        // Create a runner
        let runner = create_runner(&server, &admin.token, "Lifecycle Fail Runner").await;
        let runner_token: &str = runner.token.as_ref();

        // Create test infrastructure and a pending job
        let project_id = get_project_id(&server, project.slug.as_ref());
        let report_id = create_test_report(&server, project_id);
        let job_uuid = insert_test_job(&server, report_id);

        // Step 1: Claim the job
        let body = serde_json::json!({
            "poll_timeout": 5
        });
        let resp = server
            .client
            .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        assert_eq!(resp.status(), StatusCode::OK);
        let claimed_job: Option<JsonJob> = resp.json().await.expect("Failed to parse response");
        assert!(claimed_job.is_some());

        // Step 2: Update to running
        let body = serde_json::json!({
            "status": "running"
        });
        let resp = server
            .client
            .patch(server.api_url(&format!("/v0/runners/{}/jobs/{}", runner.uuid, job_uuid)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        assert_eq!(resp.status(), StatusCode::OK);

        // Step 3: Update to failed
        let body = serde_json::json!({
            "status": "failed",
            "exit_code": 1,
            "stderr": "Benchmark failed with error"
        });
        let resp = server
            .client
            .patch(server.api_url(&format!("/v0/runners/{}/jobs/{}", runner.uuid, job_uuid)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        assert_eq!(resp.status(), StatusCode::OK);

        // Verify final state
        let project_slug: &str = project.slug.as_ref();
        let resp = server
            .client
            .get(server.api_url(&format!("/v0/projects/{project_slug}/jobs/{job_uuid}")))
            .header("Authorization", server.bearer(&admin.token))
            .send()
            .await
            .expect("Request failed");

        assert_eq!(resp.status(), StatusCode::OK);
        let final_job: JsonJob = resp.json().await.expect("Failed to parse response");
        assert_eq!(final_job.status, JobStatus::Failed);
        assert_eq!(final_job.exit_code, Some(1));
    }

    // Test invalid state transition: claimed → completed (skipping running)
    #[tokio::test]
    async fn test_job_invalid_transition_claimed_to_completed() {
        let server = TestServer::new().await;
        let admin = server.signup("Admin", "lifecycle3@example.com").await;
        let org = server.create_org(&admin, "Invalid Trans Org").await;
        let project = server
            .create_project(&admin, &org, "Invalid Trans Project")
            .await;

        // Create a runner
        let runner = create_runner(&server, &admin.token, "Invalid Trans Runner").await;
        let runner_token: &str = runner.token.as_ref();

        // Create test infrastructure and a pending job
        let project_id = get_project_id(&server, project.slug.as_ref());
        let report_id = create_test_report(&server, project_id);
        let job_uuid = insert_test_job(&server, report_id);

        // Claim the job
        let body = serde_json::json!({
            "poll_timeout": 5
        });
        let resp = server
            .client
            .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        assert_eq!(resp.status(), StatusCode::OK);

        // Try to go directly from claimed to completed (invalid)
        let body = serde_json::json!({
            "status": "completed",
            "exit_code": 0
        });
        let resp = server
            .client
            .patch(server.api_url(&format!("/v0/runners/{}/jobs/{}", runner.uuid, job_uuid)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    // Test concurrent job claiming: two runners race for the same job, exactly one wins
    #[tokio::test]
    async fn test_concurrent_job_claim() {
        let server = TestServer::new().await;
        let admin = server.signup("Admin", "concurrent1@example.com").await;
        let org = server.create_org(&admin, "Concurrent Org").await;
        let project = server
            .create_project(&admin, &org, "Concurrent Project")
            .await;

        // Create two runners
        let runner1 = create_runner(&server, &admin.token, "Concurrent Runner 1").await;
        let runner1_token: String = runner1.token.as_ref().to_owned();
        let runner2 = create_runner(&server, &admin.token, "Concurrent Runner 2").await;
        let runner2_token: String = runner2.token.as_ref().to_owned();

        // Create a single pending job
        let project_id = get_project_id(&server, project.slug.as_ref());
        let report_id = create_test_report(&server, project_id);
        let job_uuid = insert_test_job(&server, report_id);

        // Both runners try to claim concurrently
        let claim_body = serde_json::json!({ "poll_timeout": 1 });

        let server_url_1 = server.api_url(&format!("/v0/runners/{}/jobs", runner1.uuid));
        let server_url_2 = server.api_url(&format!("/v0/runners/{}/jobs", runner2.uuid));
        let client = &server.client;

        let (resp1, resp2) = tokio::join!(
            async {
                client
                    .post(&server_url_1)
                    .header("Authorization", format!("Bearer {runner1_token}"))
                    .json(&claim_body)
                    .send()
                    .await
                    .expect("Request 1 failed")
            },
            async {
                client
                    .post(&server_url_2)
                    .header("Authorization", format!("Bearer {runner2_token}"))
                    .json(&claim_body)
                    .send()
                    .await
                    .expect("Request 2 failed")
            },
        );

        assert_eq!(resp1.status(), StatusCode::OK);
        assert_eq!(resp2.status(), StatusCode::OK);

        let job1: Option<JsonJob> = resp1.json().await.expect("Failed to parse response 1");
        let job2: Option<JsonJob> = resp2.json().await.expect("Failed to parse response 2");

        // Exactly one runner should have claimed the job
        let claimed_count = [&job1, &job2]
            .iter()
            .filter(|j| j.as_ref().is_some_and(|j| j.uuid == job_uuid))
            .count();
        assert_eq!(
            claimed_count, 1,
            "Expected exactly one runner to claim the job, but {claimed_count} claimed it"
        );

        // The other should get None (no jobs available)
        let none_count = [&job1, &job2].iter().filter(|j| j.is_none()).count();
        assert_eq!(none_count, 1, "Expected exactly one runner to get no job");
    }

    // Test that a different runner cannot update another runner's job
    #[tokio::test]
    async fn test_job_wrong_runner_update() {
        let server = TestServer::new().await;
        let admin = server.signup("Admin", "lifecycle4@example.com").await;
        let org = server.create_org(&admin, "Wrong Runner Org").await;
        let project = server
            .create_project(&admin, &org, "Wrong Runner Project")
            .await;

        // Create two runners
        let runner1 = create_runner(&server, &admin.token, "Runner One Lifecycle").await;
        let runner1_token: &str = runner1.token.as_ref();
        let runner2 = create_runner(&server, &admin.token, "Runner Two Lifecycle").await;
        let runner2_token: &str = runner2.token.as_ref();

        // Create test infrastructure and a pending job
        let project_id = get_project_id(&server, project.slug.as_ref());
        let report_id = create_test_report(&server, project_id);
        let job_uuid = insert_test_job(&server, report_id);

        // Runner 1 claims the job
        let body = serde_json::json!({
            "poll_timeout": 5
        });
        let resp = server
            .client
            .post(server.api_url(&format!("/v0/runners/{}/jobs", runner1.uuid)))
            .header("Authorization", format!("Bearer {runner1_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        assert_eq!(resp.status(), StatusCode::OK);

        // Runner 2 tries to update the job (should fail)
        let body = serde_json::json!({
            "status": "running"
        });
        let resp = server
            .client
            .patch(server.api_url(&format!("/v0/runners/{}/jobs/{}", runner2.uuid, job_uuid)))
            .header("Authorization", format!("Bearer {runner2_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }
}

// =============================================================================
// Job Spec Tests
// =============================================================================
//
// These tests verify that the job spec is correctly returned to runners

mod job_spec {
    use super::*;
    use bencher_json::{JobStatus, JsonJobSpec};
    use common::{
        insert_test_job_with_invalid_spec, insert_test_job_with_optional_fields,
        insert_test_job_with_project,
    };

    // Test that the spec is included when claiming a job
    #[tokio::test]
    async fn test_claim_job_includes_spec() {
        let server = TestServer::new().await;
        let admin = server.signup("Admin", "spec1@example.com").await;
        let org = server.create_org(&admin, "Spec Org").await;
        let project = server.create_project(&admin, &org, "Spec Project").await;

        let runner = create_runner(&server, &admin.token, "Spec Runner").await;
        let runner_token: &str = runner.token.as_ref();

        let project_id = get_project_id(&server, project.slug.as_ref());
        let report_id = create_test_report(&server, project_id);
        let _job_uuid = insert_test_job_with_project(&server, report_id, project.uuid);

        // Claim the job
        let body = serde_json::json!({ "poll_timeout": 5 });
        let resp = server
            .client
            .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        assert_eq!(resp.status(), StatusCode::OK);
        let claimed_job: Option<JsonJob> = resp.json().await.expect("Failed to parse response");
        let claimed_job = claimed_job.expect("Expected to claim a job");

        // Verify spec is present and has correct values
        let spec: &JsonJobSpec = claimed_job.spec.as_ref().expect("Expected spec to be present");
        assert_eq!(spec.project, project.uuid);
        assert_eq!(
            spec.digest.as_ref(),
            "sha256:0000000000000000000000000000000000000000000000000000000000000000"
        );
        assert_eq!(spec.vcpu, 2);
        assert_eq!(spec.memory, 4294967296); // 4 GB
        assert_eq!(spec.disk, 10737418240); // 10 GB
        assert_eq!(spec.timeout, 3600);
        assert!(!spec.network);
    }

    // Test that optional spec fields (entrypoint, cmd, env) are correctly returned
    #[tokio::test]
    async fn test_claim_job_includes_optional_spec_fields() {
        let server = TestServer::new().await;
        let admin = server.signup("Admin", "spec3@example.com").await;
        let org = server.create_org(&admin, "Spec Optional Org").await;
        let project = server
            .create_project(&admin, &org, "Spec Optional Project")
            .await;

        let runner = create_runner(&server, &admin.token, "Spec Optional Runner").await;
        let runner_token: &str = runner.token.as_ref();

        let project_id = get_project_id(&server, project.slug.as_ref());
        let report_id = create_test_report(&server, project_id);

        // Insert job with optional fields populated
        let job_uuid = insert_test_job_with_optional_fields(&server, report_id, project.uuid);

        // Claim the job
        let body = serde_json::json!({ "poll_timeout": 5 });
        let resp = server
            .client
            .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        assert_eq!(resp.status(), StatusCode::OK);
        let claimed_job: Option<JsonJob> = resp.json().await.expect("Failed to parse response");
        let claimed_job = claimed_job.expect("Expected to claim a job");
        assert_eq!(claimed_job.uuid, job_uuid);

        let spec = claimed_job.spec.as_ref().expect("Expected spec to be present");

        // Verify optional fields
        let entrypoint = spec.entrypoint.as_ref().expect("Expected entrypoint");
        assert_eq!(entrypoint, &vec!["/bin/sh".to_string(), "-c".to_string()]);

        let cmd = spec.cmd.as_ref().expect("Expected cmd");
        assert_eq!(cmd, &vec!["cargo".to_string(), "bench".to_string()]);

        let env = spec.env.as_ref().expect("Expected env");
        assert_eq!(env.get("RUST_LOG"), Some(&"info".to_string()));
        assert_eq!(env.get("CI"), Some(&"true".to_string()));
    }

    // Test that invalid spec JSON returns a 400 error
    #[tokio::test]
    async fn test_claim_job_invalid_spec_returns_error() {
        let server = TestServer::new().await;
        let admin = server.signup("Admin", "spec4@example.com").await;
        let org = server.create_org(&admin, "Spec Invalid Org").await;
        let project = server
            .create_project(&admin, &org, "Spec Invalid Project")
            .await;

        let runner = create_runner(&server, &admin.token, "Spec Invalid Runner").await;
        let runner_token: &str = runner.token.as_ref();

        let project_id = get_project_id(&server, project.slug.as_ref());
        let report_id = create_test_report(&server, project_id);

        // Insert job with invalid spec (missing required fields)
        let _job_uuid = insert_test_job_with_invalid_spec(&server, report_id);

        // Try to claim the job - should fail with 400
        let body = serde_json::json!({ "poll_timeout": 5 });
        let resp = server
            .client
            .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    // Test that the spec is NOT included in public job listing
    #[tokio::test]
    async fn test_public_job_list_excludes_spec() {
        let server = TestServer::new().await;
        let admin = server.signup("Admin", "spec2@example.com").await;
        let org = server.create_org(&admin, "Spec Public Org").await;
        let project = server
            .create_project(&admin, &org, "Spec Public Project")
            .await;

        let runner = create_runner(&server, &admin.token, "Spec Public Runner").await;
        let runner_token: &str = runner.token.as_ref();

        let project_id = get_project_id(&server, project.slug.as_ref());
        let report_id = create_test_report(&server, project_id);
        let job_uuid = insert_test_job(&server, report_id);

        // Claim the job (so it has a runner assigned)
        let body = serde_json::json!({ "poll_timeout": 5 });
        server
            .client
            .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        // Fetch the job via the public project endpoint
        let project_slug: &str = project.slug.as_ref();
        let resp = server
            .client
            .get(server.api_url(&format!(
                "/v0/projects/{project_slug}/jobs/{job_uuid}"
            )))
            .header("Authorization", server.bearer(&admin.token))
            .send()
            .await
            .expect("Request failed");

        assert_eq!(resp.status(), StatusCode::OK);
        let job: JsonJob = resp.json().await.expect("Failed to parse response");

        // Spec should NOT be included in public API response
        assert!(
            job.spec.is_none(),
            "Expected spec to be None in public API response"
        );
        assert_eq!(job.status, JobStatus::Claimed);
    }
}

// =============================================================================
// Timing Tests
// =============================================================================

// POST /v0/runners/{runner}/jobs - poll timeout respects the requested duration
#[tokio::test]
async fn test_claim_job_poll_timeout_timing() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "polltiming@example.com").await;

    let runner = create_runner(&server, &admin.token, "Poll Timing Runner").await;
    let runner_token: &str = runner.token.as_ref();

    let body = serde_json::json!({
        "poll_timeout": 2
    });

    let start = std::time::Instant::now();
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
        .header("Authorization", format!("Bearer {runner_token}"))
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    let elapsed = start.elapsed();

    assert_eq!(resp.status(), StatusCode::OK);
    let body: Option<serde_json::Value> = resp.json().await.expect("Failed to parse response");
    assert!(body.is_none(), "Expected no job to be claimed");

    // Should have waited at least 2 seconds
    assert!(
        elapsed >= std::time::Duration::from_secs(2),
        "Expected at least 2s elapsed, got {elapsed:?}"
    );
    // Should not have waited more than 4 seconds
    assert!(
        elapsed < std::time::Duration::from_secs(4),
        "Expected less than 4s elapsed, got {elapsed:?}"
    );
}

// =============================================================================
// Priority Scheduling Tests
// =============================================================================
//
// These tests verify the tier-based priority scheduling system:
// - Enterprise (>= 300) / Team (>= 200): Unlimited concurrent jobs
// - Free (>= 100): 1 concurrent job per organization
// - Unclaimed (< 100): 1 concurrent job per source IP

mod priority_scheduling {
    use super::*;
    use bencher_json::JobStatus;
    use common::{get_organization_id, insert_test_job_full};

    // Test that higher priority jobs are claimed before lower priority ones
    #[tokio::test]
    async fn test_priority_ordering() {
        let server = TestServer::new().await;
        let admin = server.signup("Admin", "priority1@example.com").await;
        let org = server.create_org(&admin, "Priority Org").await;
        let project = server
            .create_project(&admin, &org, "Priority Project")
            .await;

        let runner = create_runner(&server, &admin.token, "Priority Runner").await;
        let runner_token: &str = runner.token.as_ref();

        let project_id = get_project_id(&server, project.slug.as_ref());
        let org_id = get_organization_id(&server, project_id);
        let report_id = create_test_report(&server, project_id);

        // Insert jobs with different priorities (lower priority first to test ordering)
        let low_job = insert_test_job_full(&server, report_id, project.uuid, org_id, "10.0.0.1", 50);
        let high_job =
            insert_test_job_full(&server, report_id, project.uuid, org_id, "10.0.0.2", 300);
        let medium_job =
            insert_test_job_full(&server, report_id, project.uuid, org_id, "10.0.0.3", 150);

        // Claim first job - should be the high priority one
        let body = serde_json::json!({ "poll_timeout": 1 });
        let resp = server
            .client
            .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        assert_eq!(resp.status(), StatusCode::OK);
        let claimed: Option<JsonJob> = resp.json().await.expect("Failed to parse");
        let claimed = claimed.expect("Expected to claim a job");
        assert_eq!(claimed.uuid, high_job, "Expected high priority job to be claimed first");

        // Mark as completed so we can claim the next one
        let body = serde_json::json!({ "status": "running" });
        server
            .client
            .patch(server.api_url(&format!("/v0/runners/{}/jobs/{}", runner.uuid, high_job)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");
        let body = serde_json::json!({ "status": "completed", "exit_code": 0 });
        server
            .client
            .patch(server.api_url(&format!("/v0/runners/{}/jobs/{}", runner.uuid, high_job)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        // Claim second job - should be the medium priority one
        let body = serde_json::json!({ "poll_timeout": 1 });
        let resp = server
            .client
            .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        let claimed: Option<JsonJob> = resp.json().await.expect("Failed to parse");
        let claimed = claimed.expect("Expected to claim a job");
        assert_eq!(
            claimed.uuid, medium_job,
            "Expected medium priority job to be claimed second"
        );

        // Complete the medium job
        let body = serde_json::json!({ "status": "running" });
        server
            .client
            .patch(server.api_url(&format!("/v0/runners/{}/jobs/{}", runner.uuid, medium_job)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");
        let body = serde_json::json!({ "status": "completed", "exit_code": 0 });
        server
            .client
            .patch(server.api_url(&format!("/v0/runners/{}/jobs/{}", runner.uuid, medium_job)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        // Claim third job - should be the low priority one
        let body = serde_json::json!({ "poll_timeout": 1 });
        let resp = server
            .client
            .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        let claimed: Option<JsonJob> = resp.json().await.expect("Failed to parse");
        let claimed = claimed.expect("Expected to claim a job");
        assert_eq!(claimed.uuid, low_job, "Expected low priority job to be claimed last");
    }

    // Test Free tier concurrency limit (1 per organization)
    #[tokio::test]
    async fn test_free_tier_org_limit() {
        let server = TestServer::new().await;
        let admin = server.signup("Admin", "freetier1@example.com").await;
        let org = server.create_org(&admin, "Free Tier Org").await;
        let project = server
            .create_project(&admin, &org, "Free Tier Project")
            .await;

        let runner = create_runner(&server, &admin.token, "Free Tier Runner").await;
        let runner_token: &str = runner.token.as_ref();

        let project_id = get_project_id(&server, project.slug.as_ref());
        let org_id = get_organization_id(&server, project_id);
        let report_id = create_test_report(&server, project_id);

        // Insert two Free tier jobs (priority 100-199) for the same org
        let job1 = insert_test_job_full(&server, report_id, project.uuid, org_id, "10.0.0.1", 150);
        let _job2 = insert_test_job_full(&server, report_id, project.uuid, org_id, "10.0.0.2", 150);

        // Claim first job
        let body = serde_json::json!({ "poll_timeout": 1 });
        let resp = server
            .client
            .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        let claimed: Option<JsonJob> = resp.json().await.expect("Failed to parse");
        assert!(claimed.is_some(), "Expected to claim first job");

        // Mark job as running (not completed)
        let body = serde_json::json!({ "status": "running" });
        server
            .client
            .patch(server.api_url(&format!("/v0/runners/{}/jobs/{}", runner.uuid, job1)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        // Try to claim second job - should fail because org already has a running job
        let body = serde_json::json!({ "poll_timeout": 1 });
        let resp = server
            .client
            .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        let claimed: Option<JsonJob> = resp.json().await.expect("Failed to parse");
        assert!(
            claimed.is_none(),
            "Expected no job to be claimed due to org concurrency limit"
        );
    }

    // Test Unclaimed tier concurrency limit (1 per source IP)
    #[tokio::test]
    async fn test_unclaimed_tier_ip_limit() {
        let server = TestServer::new().await;
        let admin = server.signup("Admin", "unclaimed1@example.com").await;
        let org = server.create_org(&admin, "Unclaimed Tier Org").await;
        let project = server
            .create_project(&admin, &org, "Unclaimed Tier Project")
            .await;

        let runner = create_runner(&server, &admin.token, "Unclaimed Tier Runner").await;
        let runner_token: &str = runner.token.as_ref();

        let project_id = get_project_id(&server, project.slug.as_ref());
        let org_id = get_organization_id(&server, project_id);
        let report_id = create_test_report(&server, project_id);

        // Insert two Unclaimed tier jobs (priority < 100) with same source IP
        let same_ip = "192.168.1.100";
        let job1 = insert_test_job_full(&server, report_id, project.uuid, org_id, same_ip, 50);
        let _job2 = insert_test_job_full(&server, report_id, project.uuid, org_id, same_ip, 50);

        // Claim first job
        let body = serde_json::json!({ "poll_timeout": 1 });
        let resp = server
            .client
            .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        let claimed: Option<JsonJob> = resp.json().await.expect("Failed to parse");
        assert!(claimed.is_some(), "Expected to claim first job");

        // Mark job as running
        let body = serde_json::json!({ "status": "running" });
        server
            .client
            .patch(server.api_url(&format!("/v0/runners/{}/jobs/{}", runner.uuid, job1)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        // Try to claim second job - should fail because same IP already has a running job
        let body = serde_json::json!({ "poll_timeout": 1 });
        let resp = server
            .client
            .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        let claimed: Option<JsonJob> = resp.json().await.expect("Failed to parse");
        assert!(
            claimed.is_none(),
            "Expected no job to be claimed due to IP concurrency limit"
        );
    }

    // Test that different source IPs can run Unclaimed jobs concurrently
    #[tokio::test]
    async fn test_unclaimed_tier_different_ips() {
        let server = TestServer::new().await;
        let admin = server.signup("Admin", "unclaimed2@example.com").await;
        let org = server.create_org(&admin, "Unclaimed IPs Org").await;
        let project = server
            .create_project(&admin, &org, "Unclaimed IPs Project")
            .await;

        let runner = create_runner(&server, &admin.token, "Unclaimed IPs Runner").await;
        let runner_token: &str = runner.token.as_ref();

        let project_id = get_project_id(&server, project.slug.as_ref());
        let org_id = get_organization_id(&server, project_id);
        let report_id = create_test_report(&server, project_id);

        // Insert two Unclaimed tier jobs with DIFFERENT source IPs
        let job1 = insert_test_job_full(&server, report_id, project.uuid, org_id, "10.0.0.1", 50);
        let job2 = insert_test_job_full(&server, report_id, project.uuid, org_id, "10.0.0.2", 50);

        // Claim first job
        let body = serde_json::json!({ "poll_timeout": 1 });
        let resp = server
            .client
            .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        let claimed: Option<JsonJob> = resp.json().await.expect("Failed to parse");
        let first_claimed = claimed.expect("Expected to claim first job");

        // Mark job as running
        let body = serde_json::json!({ "status": "running" });
        server
            .client
            .patch(server.api_url(&format!(
                "/v0/runners/{}/jobs/{}",
                runner.uuid, first_claimed.uuid
            )))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        // Claim second job - should succeed because different IP
        let body = serde_json::json!({ "poll_timeout": 1 });
        let resp = server
            .client
            .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        let claimed: Option<JsonJob> = resp.json().await.expect("Failed to parse");
        let second_claimed = claimed.expect("Expected to claim second job with different IP");

        // Verify both jobs were claimed
        assert!(
            (first_claimed.uuid == job1 && second_claimed.uuid == job2)
                || (first_claimed.uuid == job2 && second_claimed.uuid == job1),
            "Expected both jobs to be claimed"
        );
    }

    // Test Enterprise/Team tier unlimited concurrency
    #[tokio::test]
    async fn test_enterprise_tier_unlimited() {
        let server = TestServer::new().await;
        let admin = server.signup("Admin", "enterprise1@example.com").await;
        let org = server.create_org(&admin, "Enterprise Tier Org").await;
        let project = server
            .create_project(&admin, &org, "Enterprise Tier Project")
            .await;

        let runner = create_runner(&server, &admin.token, "Enterprise Tier Runner").await;
        let runner_token: &str = runner.token.as_ref();

        let project_id = get_project_id(&server, project.slug.as_ref());
        let org_id = get_organization_id(&server, project_id);
        let report_id = create_test_report(&server, project_id);

        // Insert multiple Enterprise tier jobs (priority >= 200) for the same org
        let job1 = insert_test_job_full(&server, report_id, project.uuid, org_id, "10.0.0.1", 300);
        let job2 = insert_test_job_full(&server, report_id, project.uuid, org_id, "10.0.0.1", 300);

        // Claim first job
        let body = serde_json::json!({ "poll_timeout": 1 });
        let resp = server
            .client
            .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        let claimed: Option<JsonJob> = resp.json().await.expect("Failed to parse");
        let first_claimed = claimed.expect("Expected to claim first job");

        // Mark job as running (not completed)
        let body = serde_json::json!({ "status": "running" });
        server
            .client
            .patch(server.api_url(&format!(
                "/v0/runners/{}/jobs/{}",
                runner.uuid, first_claimed.uuid
            )))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        // Claim second job - should succeed because Enterprise tier has no limit
        let body = serde_json::json!({ "poll_timeout": 1 });
        let resp = server
            .client
            .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        let claimed: Option<JsonJob> = resp.json().await.expect("Failed to parse");
        let second_claimed = claimed.expect("Enterprise tier should allow concurrent jobs");

        // Verify both distinct jobs were claimed
        assert!(
            (first_claimed.uuid == job1 && second_claimed.uuid == job2)
                || (first_claimed.uuid == job2 && second_claimed.uuid == job1),
            "Expected both Enterprise tier jobs to be claimed concurrently"
        );
    }

    // Test that high priority job skips blocked lower priority jobs
    #[tokio::test]
    async fn test_high_priority_skips_blocked() {
        let server = TestServer::new().await;
        let admin = server.signup("Admin", "skipblocked@example.com").await;
        let org = server.create_org(&admin, "Skip Blocked Org").await;
        let project = server
            .create_project(&admin, &org, "Skip Blocked Project")
            .await;

        let runner = create_runner(&server, &admin.token, "Skip Blocked Runner").await;
        let runner_token: &str = runner.token.as_ref();

        let project_id = get_project_id(&server, project.slug.as_ref());
        let org_id = get_organization_id(&server, project_id);
        let report_id = create_test_report(&server, project_id);

        // Insert a Free tier job and mark it as running to block the org
        let blocking_job =
            insert_test_job_full(&server, report_id, project.uuid, org_id, "10.0.0.1", 150);

        // Claim and start the blocking job
        let body = serde_json::json!({ "poll_timeout": 1 });
        server
            .client
            .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        let body = serde_json::json!({ "status": "running" });
        server
            .client
            .patch(server.api_url(&format!(
                "/v0/runners/{}/jobs/{}",
                runner.uuid, blocking_job
            )))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        // Insert a blocked Free tier job and an unblocked Enterprise tier job
        let _blocked_job =
            insert_test_job_full(&server, report_id, project.uuid, org_id, "10.0.0.2", 150);
        let enterprise_job =
            insert_test_job_full(&server, report_id, project.uuid, org_id, "10.0.0.3", 300);

        // Try to claim - should get the Enterprise job (skipping the blocked Free tier job)
        let body = serde_json::json!({ "poll_timeout": 1 });
        let resp = server
            .client
            .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        let claimed: Option<JsonJob> = resp.json().await.expect("Failed to parse");
        let claimed = claimed.expect("Expected to claim Enterprise job");
        assert_eq!(
            claimed.uuid, enterprise_job,
            "Expected Enterprise job to be claimed, skipping blocked Free tier job"
        );
        assert_eq!(claimed.status, JobStatus::Claimed);
    }

    // Test Team tier (200-299) unlimited concurrency
    #[tokio::test]
    async fn test_team_tier_unlimited() {
        let server = TestServer::new().await;
        let admin = server.signup("Admin", "teamtier1@example.com").await;
        let org = server.create_org(&admin, "Team Tier Org").await;
        let project = server
            .create_project(&admin, &org, "Team Tier Project")
            .await;

        let runner = create_runner(&server, &admin.token, "Team Tier Runner").await;
        let runner_token: &str = runner.token.as_ref();

        let project_id = get_project_id(&server, project.slug.as_ref());
        let org_id = get_organization_id(&server, project_id);
        let report_id = create_test_report(&server, project_id);

        // Insert multiple Team tier jobs (priority 200-299) for the same org and IP
        let job1 = insert_test_job_full(&server, report_id, project.uuid, org_id, "10.0.0.1", 250);
        let job2 = insert_test_job_full(&server, report_id, project.uuid, org_id, "10.0.0.1", 250);

        // Claim first job
        let body = serde_json::json!({ "poll_timeout": 1 });
        let resp = server
            .client
            .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        let claimed: Option<JsonJob> = resp.json().await.expect("Failed to parse");
        let first_claimed = claimed.expect("Expected to claim first job");

        // Mark job as running
        let body = serde_json::json!({ "status": "running" });
        server
            .client
            .patch(server.api_url(&format!(
                "/v0/runners/{}/jobs/{}",
                runner.uuid, first_claimed.uuid
            )))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        // Claim second job - should succeed because Team tier has no limit
        let body = serde_json::json!({ "poll_timeout": 1 });
        let resp = server
            .client
            .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        let claimed: Option<JsonJob> = resp.json().await.expect("Failed to parse");
        let second_claimed = claimed.expect("Team tier should allow concurrent jobs");

        assert!(
            (first_claimed.uuid == job1 && second_claimed.uuid == job2)
                || (first_claimed.uuid == job2 && second_claimed.uuid == job1),
            "Expected both Team tier jobs to be claimed concurrently"
        );
    }

    // Test FIFO ordering within same priority level
    #[tokio::test]
    async fn test_fifo_within_same_priority() {
        let server = TestServer::new().await;
        let admin = server.signup("Admin", "fifo1@example.com").await;
        let org = server.create_org(&admin, "FIFO Org").await;
        let project = server.create_project(&admin, &org, "FIFO Project").await;

        let runner = create_runner(&server, &admin.token, "FIFO Runner").await;
        let runner_token: &str = runner.token.as_ref();

        let project_id = get_project_id(&server, project.slug.as_ref());
        let org_id = get_organization_id(&server, project_id);
        let report_id = create_test_report(&server, project_id);

        // Insert jobs with same priority - should be claimed in creation order (FIFO)
        // Use Enterprise tier so there's no concurrency blocking
        let first_job =
            insert_test_job_full(&server, report_id, project.uuid, org_id, "10.0.0.1", 300);
        // Small delay to ensure different creation timestamps
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        let second_job =
            insert_test_job_full(&server, report_id, project.uuid, org_id, "10.0.0.2", 300);
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        let third_job =
            insert_test_job_full(&server, report_id, project.uuid, org_id, "10.0.0.3", 300);

        // Claim first - should be first_job (created first)
        let body = serde_json::json!({ "poll_timeout": 1 });
        let resp = server
            .client
            .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        let claimed: Option<JsonJob> = resp.json().await.expect("Failed to parse");
        let claimed = claimed.expect("Expected to claim a job");
        assert_eq!(claimed.uuid, first_job, "Expected first created job to be claimed first");

        // Complete the job
        let body = serde_json::json!({ "status": "running" });
        server
            .client
            .patch(server.api_url(&format!("/v0/runners/{}/jobs/{}", runner.uuid, first_job)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");
        let body = serde_json::json!({ "status": "completed", "exit_code": 0 });
        server
            .client
            .patch(server.api_url(&format!("/v0/runners/{}/jobs/{}", runner.uuid, first_job)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        // Claim second - should be second_job
        let body = serde_json::json!({ "poll_timeout": 1 });
        let resp = server
            .client
            .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        let claimed: Option<JsonJob> = resp.json().await.expect("Failed to parse");
        let claimed = claimed.expect("Expected to claim a job");
        assert_eq!(claimed.uuid, second_job, "Expected second created job to be claimed second");

        // Complete second job
        let body = serde_json::json!({ "status": "running" });
        server
            .client
            .patch(server.api_url(&format!("/v0/runners/{}/jobs/{}", runner.uuid, second_job)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");
        let body = serde_json::json!({ "status": "completed", "exit_code": 0 });
        server
            .client
            .patch(server.api_url(&format!("/v0/runners/{}/jobs/{}", runner.uuid, second_job)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        // Claim third - should be third_job
        let body = serde_json::json!({ "poll_timeout": 1 });
        let resp = server
            .client
            .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        let claimed: Option<JsonJob> = resp.json().await.expect("Failed to parse");
        let claimed = claimed.expect("Expected to claim a job");
        assert_eq!(claimed.uuid, third_job, "Expected third created job to be claimed third");
    }

    // Test Free tier with different organizations can run concurrently
    #[tokio::test]
    async fn test_free_tier_different_orgs() {
        let server = TestServer::new().await;
        let admin = server.signup("Admin", "freedifforg@example.com").await;

        // Create two different organizations
        let org1 = server.create_org(&admin, "Free Tier Org 1").await;
        let org2 = server.create_org(&admin, "Free Tier Org 2").await;
        let project1 = server
            .create_project(&admin, &org1, "Free Tier Project 1")
            .await;
        let project2 = server
            .create_project(&admin, &org2, "Free Tier Project 2")
            .await;

        let runner = create_runner(&server, &admin.token, "Free Diff Org Runner").await;
        let runner_token: &str = runner.token.as_ref();

        let project1_id = get_project_id(&server, project1.slug.as_ref());
        let org1_id = get_organization_id(&server, project1_id);
        let report1_id = create_test_report(&server, project1_id);

        let project2_id = get_project_id(&server, project2.slug.as_ref());
        let org2_id = get_organization_id(&server, project2_id);
        let report2_id = create_test_report(&server, project2_id);

        // Insert Free tier jobs for different orgs
        let job1 =
            insert_test_job_full(&server, report1_id, project1.uuid, org1_id, "10.0.0.1", 150);
        let job2 =
            insert_test_job_full(&server, report2_id, project2.uuid, org2_id, "10.0.0.2", 150);

        // Claim first job
        let body = serde_json::json!({ "poll_timeout": 1 });
        let resp = server
            .client
            .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        let claimed: Option<JsonJob> = resp.json().await.expect("Failed to parse");
        let first_claimed = claimed.expect("Expected to claim first job");

        // Mark job as running
        let body = serde_json::json!({ "status": "running" });
        server
            .client
            .patch(server.api_url(&format!(
                "/v0/runners/{}/jobs/{}",
                runner.uuid, first_claimed.uuid
            )))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        // Claim second job - should succeed because different org
        let body = serde_json::json!({ "poll_timeout": 1 });
        let resp = server
            .client
            .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        let claimed: Option<JsonJob> = resp.json().await.expect("Failed to parse");
        let second_claimed =
            claimed.expect("Different orgs should allow concurrent Free tier jobs");

        assert!(
            (first_claimed.uuid == job1 && second_claimed.uuid == job2)
                || (first_claimed.uuid == job2 && second_claimed.uuid == job1),
            "Expected both Free tier jobs from different orgs to be claimed"
        );
    }

    // Test that completing a job unblocks the next job
    #[tokio::test]
    async fn test_job_completion_unblocks() {
        let server = TestServer::new().await;
        let admin = server.signup("Admin", "unblock1@example.com").await;
        let org = server.create_org(&admin, "Unblock Org").await;
        let project = server
            .create_project(&admin, &org, "Unblock Project")
            .await;

        let runner = create_runner(&server, &admin.token, "Unblock Runner").await;
        let runner_token: &str = runner.token.as_ref();

        let project_id = get_project_id(&server, project.slug.as_ref());
        let org_id = get_organization_id(&server, project_id);
        let report_id = create_test_report(&server, project_id);

        // Insert two Free tier jobs for the same org
        let job1 = insert_test_job_full(&server, report_id, project.uuid, org_id, "10.0.0.1", 150);
        let job2 = insert_test_job_full(&server, report_id, project.uuid, org_id, "10.0.0.2", 150);

        // Claim and start first job
        let body = serde_json::json!({ "poll_timeout": 1 });
        let resp = server
            .client
            .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        let claimed: Option<JsonJob> = resp.json().await.expect("Failed to parse");
        let first_claimed = claimed.expect("Expected to claim first job");

        let body = serde_json::json!({ "status": "running" });
        server
            .client
            .patch(server.api_url(&format!(
                "/v0/runners/{}/jobs/{}",
                runner.uuid, first_claimed.uuid
            )))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        // Try to claim second job - should fail (org blocked)
        let body = serde_json::json!({ "poll_timeout": 1 });
        let resp = server
            .client
            .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        let claimed: Option<JsonJob> = resp.json().await.expect("Failed to parse");
        assert!(claimed.is_none(), "Second job should be blocked while first is running");

        // Complete the first job
        let body = serde_json::json!({ "status": "completed", "exit_code": 0 });
        server
            .client
            .patch(server.api_url(&format!(
                "/v0/runners/{}/jobs/{}",
                runner.uuid, first_claimed.uuid
            )))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        // Now try to claim second job - should succeed (org no longer blocked)
        let body = serde_json::json!({ "poll_timeout": 1 });
        let resp = server
            .client
            .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        let claimed: Option<JsonJob> = resp.json().await.expect("Failed to parse");
        let second_claimed = claimed.expect("Second job should be claimable after first completes");

        // Verify we got the second job (not the completed first one)
        let expected_second = if first_claimed.uuid == job1 { job2 } else { job1 };
        assert_eq!(
            second_claimed.uuid, expected_second,
            "Expected second job to be claimed after first completes"
        );
    }

    // Test boundary values for priority tiers
    #[tokio::test]
    async fn test_priority_boundary_values() {
        let server = TestServer::new().await;
        let admin = server.signup("Admin", "boundary1@example.com").await;
        let org = server.create_org(&admin, "Boundary Org").await;
        let project = server
            .create_project(&admin, &org, "Boundary Project")
            .await;

        let runner = create_runner(&server, &admin.token, "Boundary Runner").await;
        let runner_token: &str = runner.token.as_ref();

        let project_id = get_project_id(&server, project.slug.as_ref());
        let org_id = get_organization_id(&server, project_id);
        let report_id = create_test_report(&server, project_id);

        // Test boundary: priority 199 (Free tier, should be blocked by running Free job)
        // vs priority 200 (Team tier, should NOT be blocked)
        let blocking_job =
            insert_test_job_full(&server, report_id, project.uuid, org_id, "10.0.0.1", 150);

        // Claim and start the blocking job
        let body = serde_json::json!({ "poll_timeout": 1 });
        server
            .client
            .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        let body = serde_json::json!({ "status": "running" });
        server
            .client
            .patch(server.api_url(&format!(
                "/v0/runners/{}/jobs/{}",
                runner.uuid, blocking_job
            )))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        // Insert job at priority 199 (Free tier boundary - should be blocked)
        let _free_boundary =
            insert_test_job_full(&server, report_id, project.uuid, org_id, "10.0.0.2", 199);

        // Insert job at priority 200 (Team tier boundary - should NOT be blocked)
        let team_boundary =
            insert_test_job_full(&server, report_id, project.uuid, org_id, "10.0.0.3", 200);

        // Try to claim - should get the Team tier job (priority 200), not the blocked Free tier job
        let body = serde_json::json!({ "poll_timeout": 1 });
        let resp = server
            .client
            .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        let claimed: Option<JsonJob> = resp.json().await.expect("Failed to parse");
        let claimed = claimed.expect("Expected to claim Team tier job at boundary");
        assert_eq!(
            claimed.uuid, team_boundary,
            "Priority 200 should be Team tier (unlimited), not Free tier (blocked)"
        );
    }

    // Test boundary: priority 99 vs 100 (Unclaimed vs Free tier)
    #[tokio::test]
    async fn test_priority_boundary_unclaimed_free() {
        let server = TestServer::new().await;
        let admin = server.signup("Admin", "boundary2@example.com").await;
        let org = server.create_org(&admin, "Boundary UF Org").await;
        let project = server
            .create_project(&admin, &org, "Boundary UF Project")
            .await;

        let runner = create_runner(&server, &admin.token, "Boundary UF Runner").await;
        let runner_token: &str = runner.token.as_ref();

        let project_id = get_project_id(&server, project.slug.as_ref());
        let org_id = get_organization_id(&server, project_id);
        let report_id = create_test_report(&server, project_id);

        // Start a job with same source IP to block Unclaimed tier
        let same_ip = "192.168.1.50";
        let blocking_job =
            insert_test_job_full(&server, report_id, project.uuid, org_id, same_ip, 50);

        // Claim and start the blocking job
        let body = serde_json::json!({ "poll_timeout": 1 });
        server
            .client
            .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        let body = serde_json::json!({ "status": "running" });
        server
            .client
            .patch(server.api_url(&format!(
                "/v0/runners/{}/jobs/{}",
                runner.uuid, blocking_job
            )))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        // Insert job at priority 99 with same IP (Unclaimed tier - should be blocked by IP)
        let _unclaimed_boundary =
            insert_test_job_full(&server, report_id, project.uuid, org_id, same_ip, 99);

        // Insert job at priority 100 (Free tier - blocked by org, not IP)
        // But since org already has a running job, this should also be blocked
        let _free_boundary =
            insert_test_job_full(&server, report_id, project.uuid, org_id, same_ip, 100);

        // Insert a Free tier job with DIFFERENT IP and DIFFERENT org to prove Free tier works
        let org2 = server.create_org(&admin, "Boundary UF Org 2").await;
        let project2 = server
            .create_project(&admin, &org2, "Boundary UF Project 2")
            .await;
        let project2_id = get_project_id(&server, project2.slug.as_ref());
        let org2_id = get_organization_id(&server, project2_id);
        let report2_id = create_test_report(&server, project2_id);

        let free_unblocked =
            insert_test_job_full(&server, report2_id, project2.uuid, org2_id, "10.0.0.99", 100);

        // Try to claim - should get the Free tier job from different org
        let body = serde_json::json!({ "poll_timeout": 1 });
        let resp = server
            .client
            .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        let claimed: Option<JsonJob> = resp.json().await.expect("Failed to parse");
        let claimed = claimed.expect("Expected to claim unblocked Free tier job");
        assert_eq!(
            claimed.uuid, free_unblocked,
            "Priority 100 from different org should be claimable"
        );
    }
}
