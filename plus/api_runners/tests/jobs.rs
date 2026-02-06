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
