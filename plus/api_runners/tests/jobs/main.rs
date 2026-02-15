#![expect(unused_crate_dependencies, clippy::tests_outside_test_module)]
//! Integration tests for runner agent job endpoints.
//!
//! Note: These tests require a job to exist in the database.
//! Since jobs are tied to reports, which require projects and other setup,
//! some tests may be marked as ignored until full integration is available.

#[path = "../common/mod.rs"]
mod common;
mod websocket;

use std::sync::Arc;

use bencher_api_tests::TestServer;
use bencher_json::{DateTime, JobPriority, JobStatus, JsonClaimedJob, JsonJob};
use bencher_schema::{
    context::HeartbeatTasks,
    model::runner::{JobId, recover_orphaned_claimed_jobs, spawn_heartbeat_timeout},
    schema,
};
use common::{
    associate_runner_spec, create_runner, create_test_report, get_job_priority, get_project_id,
    get_runner_id, insert_test_job, insert_test_job_full, insert_test_job_with_invalid_config,
    insert_test_spec, insert_test_spec_full, set_job_runner_id, set_job_status,
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use futures_concurrency::future::Join as _;
use http::StatusCode;
use tokio::sync::Mutex;

// POST /v0/runners/{runner}/jobs - claim job with valid token (no jobs available)
#[tokio::test]
async fn claim_job_no_jobs() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "jobsadmin@example.com").await;

    let (_, spec_id) = insert_test_spec(&server);
    let runner = create_runner(&server, &admin.token, "Claim Test Runner").await;
    let runner_token: &str = runner.token.as_ref();
    let runner_id = get_runner_id(&server, runner.uuid);
    associate_runner_spec(&server, runner_id, spec_id);

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
async fn claim_job_invalid_token() {
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

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// POST /v0/runners/{runner}/jobs - token for wrong runner rejected
#[tokio::test]
async fn claim_job_wrong_runner_token() {
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

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// POST /v0/runners/{runner}/jobs - missing Authorization header
#[tokio::test]
async fn claim_job_missing_auth() {
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

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// POST /v0/runners/{runner}/jobs - token without prefix rejected
#[tokio::test]
async fn claim_job_no_prefix_token() {
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

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// POST /v0/runners/{runner}/jobs - correct prefix but wrong length rejected
#[tokio::test]
async fn claim_job_wrong_length_token() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "jobsadmin-wl@example.com").await;

    let runner = create_runner(&server, &admin.token, "Wrong Length Runner").await;

    let body = serde_json::json!({
        "poll_timeout": 1
    });

    // Token has the correct prefix but is too short (15 + 32 hex = 47 chars, not 79)
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
        .header(
            "Authorization",
            "Bearer bencher_runner_00112233445566778899aabbccddeeff",
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // Token has the correct prefix but is too long (15 + 66 hex = 81 chars, not 79)
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
        .header(
            "Authorization",
            "Bearer bencher_runner_00112233445566778899aabbccddeeff00112233445566778899aabbccddeeffab",
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// POST /v0/runners/{runner}/jobs - archived runner rejected
#[tokio::test]
async fn claim_job_archived_runner() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "jobsadmin9@example.com").await;

    let (_, spec_id) = insert_test_spec(&server);
    let runner = create_runner(&server, &admin.token, "Archived Runner").await;
    let runner_token: &str = runner.token.as_ref();
    let runner_id = get_runner_id(&server, runner.uuid);
    associate_runner_spec(&server, runner_id, spec_id);

    // Archive the runner
    let body = serde_json::json!({
        "archived": true
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

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// POST /v0/runners/{runner}/jobs - canceled pending job is not claimable
#[tokio::test]
async fn claim_job_canceled_pending() {
    let server = TestServer::new().await;
    let admin = server
        .signup("Admin", "jobsadmin-canceled@example.com")
        .await;
    let org = server.create_org(&admin, "Canceled Pending Org").await;
    let project = server
        .create_project(&admin, &org, "Canceled Pending Project")
        .await;

    let (_, spec_id) = insert_test_spec(&server);
    let runner = create_runner(&server, &admin.token, "Canceled Pending Runner").await;
    let runner_token: &str = runner.token.as_ref();
    let runner_id = get_runner_id(&server, runner.uuid);
    associate_runner_spec(&server, runner_id, spec_id);

    // Create test infrastructure and a pending job
    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let job_uuid = insert_test_job(&server, report_id, spec_id);

    // Cancel the pending job before claiming
    set_job_status(&server, job_uuid, JobStatus::Canceled);

    // Try to claim - should get None since only Pending jobs are claimable
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
    let claimed: Option<JsonClaimedJob> = resp.json().await.expect("Failed to parse response");
    assert!(
        claimed.is_none(),
        "Canceled pending job should not be claimable"
    );
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
    async fn job_lifecycle_completed() {
        let server = TestServer::new().await;
        let admin = server.signup("Admin", "lifecycle1@example.com").await;
        let org = server.create_org(&admin, "Lifecycle Org").await;
        let project = server
            .create_project(&admin, &org, "Lifecycle Project")
            .await;

        // Create a runner
        let (_, spec_id) = insert_test_spec(&server);
        let runner = create_runner(&server, &admin.token, "Lifecycle Runner").await;
        let runner_token: &str = runner.token.as_ref();
        let runner_id = get_runner_id(&server, runner.uuid);
        associate_runner_spec(&server, runner_id, spec_id);

        // Create test infrastructure and a pending job
        let project_id = get_project_id(&server, project.slug.as_ref());
        let report_id = create_test_report(&server, project_id);
        let job_uuid = insert_test_job(&server, report_id, spec_id);

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
        let claimed_job: Option<JsonClaimedJob> =
            resp.json().await.expect("Failed to parse response");
        let claimed_job = claimed_job.expect("Expected to claim a job");
        assert_eq!(claimed_job.uuid, job_uuid);

        // Step 2: Transition to running via DB
        set_job_status(&server, job_uuid, JobStatus::Running);

        // Step 3: Transition to completed via DB
        set_job_status(&server, job_uuid, JobStatus::Completed);

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
        assert_eq!(final_job.runner, Some(runner.uuid));
        assert!(final_job.claimed.is_some());
    }

    // Test the job lifecycle with failure: claim → running → failed
    #[tokio::test]
    async fn job_lifecycle_failed() {
        let server = TestServer::new().await;
        let admin = server.signup("Admin", "lifecycle2@example.com").await;
        let org = server.create_org(&admin, "Lifecycle Fail Org").await;
        let project = server
            .create_project(&admin, &org, "Lifecycle Fail Project")
            .await;

        // Create a runner
        let (_, spec_id) = insert_test_spec(&server);
        let runner = create_runner(&server, &admin.token, "Lifecycle Fail Runner").await;
        let runner_token: &str = runner.token.as_ref();
        let runner_id = get_runner_id(&server, runner.uuid);
        associate_runner_spec(&server, runner_id, spec_id);

        // Create test infrastructure and a pending job
        let project_id = get_project_id(&server, project.slug.as_ref());
        let report_id = create_test_report(&server, project_id);
        let job_uuid = insert_test_job(&server, report_id, spec_id);

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
        let claimed_job: Option<JsonClaimedJob> =
            resp.json().await.expect("Failed to parse response");
        assert!(claimed_job.is_some());

        // Step 2: Transition to running via DB
        set_job_status(&server, job_uuid, JobStatus::Running);

        // Step 3: Transition to failed via DB
        set_job_status(&server, job_uuid, JobStatus::Failed);

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
        assert_eq!(final_job.runner, Some(runner.uuid));
        assert!(final_job.claimed.is_some());
    }

    // Test concurrent job claiming: two runners race for the same job, exactly one wins
    #[tokio::test]
    async fn concurrent_job_claim() {
        let server = TestServer::new().await;
        let admin = server.signup("Admin", "concurrent1@example.com").await;
        let org = server.create_org(&admin, "Concurrent Org").await;
        let project = server
            .create_project(&admin, &org, "Concurrent Project")
            .await;

        // Create two runners
        let (_, spec_id) = insert_test_spec(&server);
        let runner1 = create_runner(&server, &admin.token, "Concurrent Runner 1").await;
        let runner1_token: String = runner1.token.as_ref().to_owned();
        let runner1_id = get_runner_id(&server, runner1.uuid);
        associate_runner_spec(&server, runner1_id, spec_id);
        let runner2 = create_runner(&server, &admin.token, "Concurrent Runner 2").await;
        let runner2_token: String = runner2.token.as_ref().to_owned();
        let runner2_id = get_runner_id(&server, runner2.uuid);
        associate_runner_spec(&server, runner2_id, spec_id);

        // Create a single pending job
        let project_id = get_project_id(&server, project.slug.as_ref());
        let report_id = create_test_report(&server, project_id);
        let job_uuid = insert_test_job(&server, report_id, spec_id);

        // Both runners try to claim concurrently
        let claim_body = serde_json::json!({ "poll_timeout": 1 });

        let server_url_1 = server.api_url(&format!("/v0/runners/{}/jobs", runner1.uuid));
        let server_url_2 = server.api_url(&format!("/v0/runners/{}/jobs", runner2.uuid));
        let client = &server.client;

        let (resp1, resp2) = (
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
        )
            .join()
            .await;

        assert_eq!(resp1.status(), StatusCode::OK);
        assert_eq!(resp2.status(), StatusCode::OK);

        let job1: Option<JsonClaimedJob> = resp1.json().await.expect("Failed to parse response 1");
        let job2: Option<JsonClaimedJob> = resp2.json().await.expect("Failed to parse response 2");

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
}

// =============================================================================
// Job Spec Tests
// =============================================================================
//
// These tests verify that the job spec is correctly returned to runners

mod job_spec {
    use super::*;
    use bencher_json::{JobStatus, JsonJobConfig};
    use common::{
        insert_test_job_with_invalid_config, insert_test_job_with_optional_fields,
        insert_test_job_with_project,
    };

    // Test that the spec is included when claiming a job
    #[tokio::test]
    async fn claim_job_includes_spec() {
        let server = TestServer::new().await;
        let admin = server.signup("Admin", "spec1@example.com").await;
        let org = server.create_org(&admin, "Spec Org").await;
        let project = server.create_project(&admin, &org, "Spec Project").await;

        let (_, spec_id) = insert_test_spec(&server);
        let runner = create_runner(&server, &admin.token, "Spec Runner").await;
        let runner_token: &str = runner.token.as_ref();
        let runner_id = get_runner_id(&server, runner.uuid);
        associate_runner_spec(&server, runner_id, spec_id);

        let project_id = get_project_id(&server, project.slug.as_ref());
        let report_id = create_test_report(&server, project_id);
        let _job_uuid = insert_test_job_with_project(&server, report_id, project.uuid, spec_id);

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
        let claimed_job: Option<JsonClaimedJob> =
            resp.json().await.expect("Failed to parse response");
        let claimed_job = claimed_job.expect("Expected to claim a job");

        // Verify spec is present and has correct values
        assert_eq!(u32::from(claimed_job.spec.cpu), 2);
        assert_eq!(u64::from(claimed_job.spec.memory), 0x0001_0000_0000); // 4 GB
        assert_eq!(u64::from(claimed_job.spec.disk), 10_737_418_240); // 10 GB
        assert!(!claimed_job.spec.network);

        // Verify config has correct values
        let config: &JsonJobConfig = &claimed_job.config;
        assert_eq!(config.project, project.uuid);
        assert_eq!(
            config.digest.as_ref(),
            "sha256:0000000000000000000000000000000000000000000000000000000000000000"
        );
        assert_eq!(u32::from(config.timeout), 3600);
        assert!(config.file_paths.is_none());
    }

    // Test that optional spec fields (entrypoint, cmd, env) are correctly returned
    #[tokio::test]
    async fn claim_job_includes_optional_spec_fields() {
        let server = TestServer::new().await;
        let admin = server.signup("Admin", "spec3@example.com").await;
        let org = server.create_org(&admin, "Spec Optional Org").await;
        let project = server
            .create_project(&admin, &org, "Spec Optional Project")
            .await;

        let (_, spec_id) = insert_test_spec(&server);
        let runner = create_runner(&server, &admin.token, "Spec Optional Runner").await;
        let runner_token: &str = runner.token.as_ref();
        let runner_id = get_runner_id(&server, runner.uuid);
        associate_runner_spec(&server, runner_id, spec_id);

        let project_id = get_project_id(&server, project.slug.as_ref());
        let report_id = create_test_report(&server, project_id);

        // Insert job with optional fields populated
        let job_uuid =
            insert_test_job_with_optional_fields(&server, report_id, project.uuid, spec_id);

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
        let claimed_job: Option<JsonClaimedJob> =
            resp.json().await.expect("Failed to parse response");
        let claimed_job = claimed_job.expect("Expected to claim a job");
        assert_eq!(claimed_job.uuid, job_uuid);

        let config = &claimed_job.config;

        // Verify optional fields
        let entrypoint = config.entrypoint.as_ref().expect("Expected entrypoint");
        assert_eq!(entrypoint, &vec!["/bin/sh".to_owned(), "-c".to_owned()]);

        let cmd = config.cmd.as_ref().expect("Expected cmd");
        assert_eq!(cmd, &vec!["cargo".to_owned(), "bench".to_owned()]);

        let env = config.env.as_ref().expect("Expected env");
        assert_eq!(env.get("RUST_LOG"), Some(&"info".to_owned()));
        assert_eq!(env.get("CI"), Some(&"true".to_owned()));

        let file_paths: Vec<&str> = config
            .file_paths
            .as_ref()
            .expect("Expected file_paths")
            .iter()
            .map(|p| p.as_str())
            .collect();
        assert_eq!(file_paths, vec!["/output/results.json", "/tmp/bench.txt"]);
    }

    // Test that invalid spec JSON returns a 400 error
    #[tokio::test]
    async fn claim_job_invalid_spec_returns_error() {
        let server = TestServer::new().await;
        let admin = server.signup("Admin", "spec4@example.com").await;
        let org = server.create_org(&admin, "Spec Invalid Org").await;
        let project = server
            .create_project(&admin, &org, "Spec Invalid Project")
            .await;

        let (_, spec_id) = insert_test_spec(&server);
        let runner = create_runner(&server, &admin.token, "Spec Invalid Runner").await;
        let runner_token: &str = runner.token.as_ref();
        let runner_id = get_runner_id(&server, runner.uuid);
        associate_runner_spec(&server, runner_id, spec_id);

        let project_id = get_project_id(&server, project.slug.as_ref());
        let report_id = create_test_report(&server, project_id);

        // Insert job with invalid config (missing required fields)
        let _job_uuid = insert_test_job_with_invalid_config(&server, report_id, spec_id);

        // Try to claim the job - should fail with 500 (corrupt spec is a server error)
        let body = serde_json::json!({ "poll_timeout": 5 });
        let resp = server
            .client
            .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    // Test that the config is NOT included in public job listing
    #[tokio::test]
    async fn public_job_list_excludes_spec() {
        let server = TestServer::new().await;
        let admin = server.signup("Admin", "spec2@example.com").await;
        let org = server.create_org(&admin, "Spec Public Org").await;
        let project = server
            .create_project(&admin, &org, "Spec Public Project")
            .await;

        let (_, spec_id) = insert_test_spec(&server);
        let runner = create_runner(&server, &admin.token, "Spec Public Runner").await;
        let runner_token: &str = runner.token.as_ref();
        let runner_id = get_runner_id(&server, runner.uuid);
        associate_runner_spec(&server, runner_id, spec_id);

        let project_id = get_project_id(&server, project.slug.as_ref());
        let report_id = create_test_report(&server, project_id);
        let job_uuid = insert_test_job(&server, report_id, spec_id);

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
            .get(server.api_url(&format!("/v0/projects/{project_slug}/jobs/{job_uuid}")))
            .header("Authorization", server.bearer(&admin.token))
            .send()
            .await
            .expect("Request failed");

        assert_eq!(resp.status(), StatusCode::OK);
        let job: JsonJob = resp.json().await.expect("Failed to parse response");

        // Config should NOT be included in public API response
        assert!(
            job.config.is_none(),
            "Expected config to be None in public API response"
        );
        assert_eq!(job.status, JobStatus::Claimed);
    }
}

// =============================================================================
// Timing Tests
// =============================================================================

// POST /v0/runners/{runner}/jobs - poll timeout respects the requested duration
#[tokio::test]
async fn claim_job_poll_timeout_timing() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "polltiming@example.com").await;

    let (_, spec_id) = insert_test_spec(&server);
    let runner = create_runner(&server, &admin.token, "Poll Timing Runner").await;
    let runner_token: &str = runner.token.as_ref();
    let runner_id = get_runner_id(&server, runner.uuid);
    associate_runner_spec(&server, runner_id, spec_id);

    tokio::time::pause();
    let handle = tokio::spawn({
        let client = server.client.clone();
        let url = server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid));
        let token = runner_token.to_owned();
        async move {
            client
                .post(url)
                .header("Authorization", format!("Bearer {token}"))
                .json(&serde_json::json!({ "poll_timeout": 2 }))
                .send()
                .await
                .expect("Request failed")
        }
    });

    // Advance time past the 2-second poll timeout
    tokio::time::advance(std::time::Duration::from_secs(3)).await;

    let resp = handle.await.expect("Task panicked");
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Option<serde_json::Value> = resp.json().await.expect("Failed to parse response");
    assert!(body.is_none(), "Expected no job to be claimed");
}

// =============================================================================
// Priority Scheduling Tests
// =============================================================================
//
// These tests verify the tier-based priority scheduling system:
// - Enterprise / Team: Unlimited concurrent jobs
// - Free: 1 concurrent job per organization
// - Unclaimed: 1 concurrent job per source IP

mod priority_scheduling {
    use super::*;
    use bencher_json::{DateTime, JobPriority, JobStatus};
    use common::{get_organization_id, insert_test_job_full, insert_test_job_with_timestamp};

    // Test that higher priority jobs are claimed before lower priority ones
    #[tokio::test]
    async fn priority_ordering() {
        let server = TestServer::new().await;
        let admin = server.signup("Admin", "priority1@example.com").await;
        let org = server.create_org(&admin, "Priority Org").await;
        let project = server
            .create_project(&admin, &org, "Priority Project")
            .await;

        let (_, spec_id) = insert_test_spec(&server);
        let runner = create_runner(&server, &admin.token, "Priority Runner").await;
        let runner_token: &str = runner.token.as_ref();
        let runner_id = get_runner_id(&server, runner.uuid);
        associate_runner_spec(&server, runner_id, spec_id);

        let project_id = get_project_id(&server, project.slug.as_ref());
        let org_id = get_organization_id(&server, project_id);
        let report_id = create_test_report(&server, project_id);

        // Insert jobs with different priorities (lower priority first to test ordering)
        let low_job = insert_test_job_full(
            &server,
            report_id,
            project.uuid,
            org_id,
            "10.0.0.1",
            JobPriority::Unclaimed,
            spec_id,
        );
        let high_job = insert_test_job_full(
            &server,
            report_id,
            project.uuid,
            org_id,
            "10.0.0.2",
            JobPriority::Enterprise,
            spec_id,
        );
        let medium_job = insert_test_job_full(
            &server,
            report_id,
            project.uuid,
            org_id,
            "10.0.0.3",
            JobPriority::Free,
            spec_id,
        );

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
        let claimed: Option<JsonClaimedJob> = resp.json().await.expect("Failed to parse");
        let claimed = claimed.expect("Expected to claim a job");
        assert_eq!(
            claimed.uuid, high_job,
            "Expected high priority job to be claimed first"
        );

        // Mark as completed so we can claim the next one
        set_job_status(&server, high_job, JobStatus::Running);
        set_job_status(&server, high_job, JobStatus::Completed);

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

        let claimed: Option<JsonClaimedJob> = resp.json().await.expect("Failed to parse");
        let claimed = claimed.expect("Expected to claim a job");
        assert_eq!(
            claimed.uuid, medium_job,
            "Expected medium priority job to be claimed second"
        );

        // Complete the medium job
        set_job_status(&server, medium_job, JobStatus::Running);
        set_job_status(&server, medium_job, JobStatus::Completed);

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

        let claimed: Option<JsonClaimedJob> = resp.json().await.expect("Failed to parse");
        let claimed = claimed.expect("Expected to claim a job");
        assert_eq!(
            claimed.uuid, low_job,
            "Expected low priority job to be claimed last"
        );
    }

    // Test Free tier concurrency limit (1 per organization)
    #[tokio::test]
    async fn free_tier_org_limit() {
        let server = TestServer::new().await;
        let admin = server.signup("Admin", "freetier1@example.com").await;
        let org = server.create_org(&admin, "Free Tier Org").await;
        let project = server
            .create_project(&admin, &org, "Free Tier Project")
            .await;

        let (_, spec_id) = insert_test_spec(&server);
        let runner = create_runner(&server, &admin.token, "Free Tier Runner").await;
        let runner_token: &str = runner.token.as_ref();
        let runner_id = get_runner_id(&server, runner.uuid);
        associate_runner_spec(&server, runner_id, spec_id);

        let project_id = get_project_id(&server, project.slug.as_ref());
        let org_id = get_organization_id(&server, project_id);
        let report_id = create_test_report(&server, project_id);

        // Insert two Free tier jobs for the same org
        let job1 = insert_test_job_full(
            &server,
            report_id,
            project.uuid,
            org_id,
            "10.0.0.1",
            JobPriority::Free,
            spec_id,
        );
        let _job2 = insert_test_job_full(
            &server,
            report_id,
            project.uuid,
            org_id,
            "10.0.0.2",
            JobPriority::Free,
            spec_id,
        );

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

        let claimed: Option<JsonClaimedJob> = resp.json().await.expect("Failed to parse");
        assert!(claimed.is_some(), "Expected to claim first job");

        // Mark job as running (not completed)
        set_job_status(&server, job1, JobStatus::Running);

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

        let claimed: Option<JsonClaimedJob> = resp.json().await.expect("Failed to parse");
        assert!(
            claimed.is_none(),
            "Expected no job to be claimed due to org concurrency limit"
        );
    }

    // Test Unclaimed tier concurrency limit (1 per source IP)
    #[tokio::test]
    async fn unclaimed_tier_ip_limit() {
        let server = TestServer::new().await;
        let admin = server.signup("Admin", "unclaimed1@example.com").await;
        let org = server.create_org(&admin, "Unclaimed Tier Org").await;
        let project = server
            .create_project(&admin, &org, "Unclaimed Tier Project")
            .await;

        let (_, spec_id) = insert_test_spec(&server);
        let runner = create_runner(&server, &admin.token, "Unclaimed Tier Runner").await;
        let runner_token: &str = runner.token.as_ref();
        let runner_id = get_runner_id(&server, runner.uuid);
        associate_runner_spec(&server, runner_id, spec_id);

        let project_id = get_project_id(&server, project.slug.as_ref());
        let org_id = get_organization_id(&server, project_id);
        let report_id = create_test_report(&server, project_id);

        // Insert two Unclaimed tier jobs with same source IP
        let same_ip = "192.168.1.100";
        let job1 = insert_test_job_full(
            &server,
            report_id,
            project.uuid,
            org_id,
            same_ip,
            JobPriority::Unclaimed,
            spec_id,
        );
        let _job2 = insert_test_job_full(
            &server,
            report_id,
            project.uuid,
            org_id,
            same_ip,
            JobPriority::Unclaimed,
            spec_id,
        );

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

        let claimed: Option<JsonClaimedJob> = resp.json().await.expect("Failed to parse");
        assert!(claimed.is_some(), "Expected to claim first job");

        // Mark job as running
        set_job_status(&server, job1, JobStatus::Running);

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

        let claimed: Option<JsonClaimedJob> = resp.json().await.expect("Failed to parse");
        assert!(
            claimed.is_none(),
            "Expected no job to be claimed due to IP concurrency limit"
        );
    }

    // Test that different source IPs can run Unclaimed jobs concurrently
    #[tokio::test]
    async fn unclaimed_tier_different_ips() {
        let server = TestServer::new().await;
        let admin = server.signup("Admin", "unclaimed2@example.com").await;
        let org = server.create_org(&admin, "Unclaimed IPs Org").await;
        let project = server
            .create_project(&admin, &org, "Unclaimed IPs Project")
            .await;

        let (_, spec_id) = insert_test_spec(&server);
        let runner = create_runner(&server, &admin.token, "Unclaimed IPs Runner").await;
        let runner_token: &str = runner.token.as_ref();
        let runner_id = get_runner_id(&server, runner.uuid);
        associate_runner_spec(&server, runner_id, spec_id);

        let project_id = get_project_id(&server, project.slug.as_ref());
        let org_id = get_organization_id(&server, project_id);
        let report_id = create_test_report(&server, project_id);

        // Insert two Unclaimed tier jobs with DIFFERENT source IPs
        let job1 = insert_test_job_full(
            &server,
            report_id,
            project.uuid,
            org_id,
            "10.0.0.1",
            JobPriority::Unclaimed,
            spec_id,
        );
        let job2 = insert_test_job_full(
            &server,
            report_id,
            project.uuid,
            org_id,
            "10.0.0.2",
            JobPriority::Unclaimed,
            spec_id,
        );

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

        let claimed: Option<JsonClaimedJob> = resp.json().await.expect("Failed to parse");
        let first_claimed = claimed.expect("Expected to claim first job");

        // Mark job as running
        set_job_status(&server, first_claimed.uuid, JobStatus::Running);

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

        let claimed: Option<JsonClaimedJob> = resp.json().await.expect("Failed to parse");
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
    async fn enterprise_tier_unlimited() {
        let server = TestServer::new().await;
        let admin = server.signup("Admin", "enterprise1@example.com").await;
        let org = server.create_org(&admin, "Enterprise Tier Org").await;
        let project = server
            .create_project(&admin, &org, "Enterprise Tier Project")
            .await;

        let (_, spec_id) = insert_test_spec(&server);
        let runner = create_runner(&server, &admin.token, "Enterprise Tier Runner").await;
        let runner_token: &str = runner.token.as_ref();
        let runner_id = get_runner_id(&server, runner.uuid);
        associate_runner_spec(&server, runner_id, spec_id);

        let project_id = get_project_id(&server, project.slug.as_ref());
        let org_id = get_organization_id(&server, project_id);
        let report_id = create_test_report(&server, project_id);

        // Insert multiple Enterprise tier jobs for the same org
        let job1 = insert_test_job_full(
            &server,
            report_id,
            project.uuid,
            org_id,
            "10.0.0.1",
            JobPriority::Enterprise,
            spec_id,
        );
        let job2 = insert_test_job_full(
            &server,
            report_id,
            project.uuid,
            org_id,
            "10.0.0.1",
            JobPriority::Enterprise,
            spec_id,
        );

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

        let claimed: Option<JsonClaimedJob> = resp.json().await.expect("Failed to parse");
        let first_claimed = claimed.expect("Expected to claim first job");

        // Mark job as running (not completed)
        set_job_status(&server, first_claimed.uuid, JobStatus::Running);

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

        let claimed: Option<JsonClaimedJob> = resp.json().await.expect("Failed to parse");
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
    async fn high_priority_skips_blocked() {
        let server = TestServer::new().await;
        let admin = server.signup("Admin", "skipblocked@example.com").await;
        let org = server.create_org(&admin, "Skip Blocked Org").await;
        let project = server
            .create_project(&admin, &org, "Skip Blocked Project")
            .await;

        let (_, spec_id) = insert_test_spec(&server);
        let runner = create_runner(&server, &admin.token, "Skip Blocked Runner").await;
        let runner_token: &str = runner.token.as_ref();
        let runner_id = get_runner_id(&server, runner.uuid);
        associate_runner_spec(&server, runner_id, spec_id);

        let project_id = get_project_id(&server, project.slug.as_ref());
        let org_id = get_organization_id(&server, project_id);
        let report_id = create_test_report(&server, project_id);

        // Insert a Free tier job and mark it as running to block the org
        let blocking_job = insert_test_job_full(
            &server,
            report_id,
            project.uuid,
            org_id,
            "10.0.0.1",
            JobPriority::Free,
            spec_id,
        );

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

        set_job_status(&server, blocking_job, JobStatus::Running);

        // Insert a blocked Free tier job and an unblocked Enterprise tier job
        let _blocked_job = insert_test_job_full(
            &server,
            report_id,
            project.uuid,
            org_id,
            "10.0.0.2",
            JobPriority::Free,
            spec_id,
        );
        let enterprise_job = insert_test_job_full(
            &server,
            report_id,
            project.uuid,
            org_id,
            "10.0.0.3",
            JobPriority::Enterprise,
            spec_id,
        );

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

        let claimed: Option<JsonClaimedJob> = resp.json().await.expect("Failed to parse");
        let claimed = claimed.expect("Expected to claim Enterprise job");
        assert_eq!(
            claimed.uuid, enterprise_job,
            "Expected Enterprise job to be claimed, skipping blocked Free tier job"
        );
    }

    // Test Team tier (200-299) unlimited concurrency
    #[tokio::test]
    async fn team_tier_unlimited() {
        let server = TestServer::new().await;
        let admin = server.signup("Admin", "teamtier1@example.com").await;
        let org = server.create_org(&admin, "Team Tier Org").await;
        let project = server
            .create_project(&admin, &org, "Team Tier Project")
            .await;

        let (_, spec_id) = insert_test_spec(&server);
        let runner = create_runner(&server, &admin.token, "Team Tier Runner").await;
        let runner_token: &str = runner.token.as_ref();
        let runner_id = get_runner_id(&server, runner.uuid);
        associate_runner_spec(&server, runner_id, spec_id);

        let project_id = get_project_id(&server, project.slug.as_ref());
        let org_id = get_organization_id(&server, project_id);
        let report_id = create_test_report(&server, project_id);

        // Insert multiple Team tier jobs for the same org and IP
        let job1 = insert_test_job_full(
            &server,
            report_id,
            project.uuid,
            org_id,
            "10.0.0.1",
            JobPriority::Team,
            spec_id,
        );
        let job2 = insert_test_job_full(
            &server,
            report_id,
            project.uuid,
            org_id,
            "10.0.0.1",
            JobPriority::Team,
            spec_id,
        );

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

        let claimed: Option<JsonClaimedJob> = resp.json().await.expect("Failed to parse");
        let first_claimed = claimed.expect("Expected to claim first job");

        // Mark job as running
        set_job_status(&server, first_claimed.uuid, JobStatus::Running);

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

        let claimed: Option<JsonClaimedJob> = resp.json().await.expect("Failed to parse");
        let second_claimed = claimed.expect("Team tier should allow concurrent jobs");

        assert!(
            (first_claimed.uuid == job1 && second_claimed.uuid == job2)
                || (first_claimed.uuid == job2 && second_claimed.uuid == job1),
            "Expected both Team tier jobs to be claimed concurrently"
        );
    }

    // Test FIFO ordering within same priority level
    #[tokio::test]
    async fn fifo_within_same_priority() {
        let server = TestServer::new().await;
        let admin = server.signup("Admin", "fifo1@example.com").await;
        let org = server.create_org(&admin, "FIFO Org").await;
        let project = server.create_project(&admin, &org, "FIFO Project").await;

        let (_, spec_id) = insert_test_spec(&server);
        let runner = create_runner(&server, &admin.token, "FIFO Runner").await;
        let runner_token: &str = runner.token.as_ref();
        let runner_id = get_runner_id(&server, runner.uuid);
        associate_runner_spec(&server, runner_id, spec_id);

        let project_id = get_project_id(&server, project.slug.as_ref());
        let org_id = get_organization_id(&server, project_id);
        let report_id = create_test_report(&server, project_id);

        // Insert jobs with same priority - should be claimed in creation order (FIFO)
        // Use Enterprise tier so there's no concurrency blocking
        // Use explicit timestamps to guarantee deterministic ordering
        let base_ts = DateTime::now();
        let ts1 = base_ts;
        let ts2 = DateTime::try_from(base_ts.timestamp() + 1).unwrap();
        let ts3 = DateTime::try_from(base_ts.timestamp() + 2).unwrap();
        let first_job = insert_test_job_with_timestamp(
            &server,
            report_id,
            project.uuid,
            org_id,
            "10.0.0.1",
            JobPriority::Enterprise,
            ts1,
            spec_id,
        );
        let second_job = insert_test_job_with_timestamp(
            &server,
            report_id,
            project.uuid,
            org_id,
            "10.0.0.2",
            JobPriority::Enterprise,
            ts2,
            spec_id,
        );
        let third_job = insert_test_job_with_timestamp(
            &server,
            report_id,
            project.uuid,
            org_id,
            "10.0.0.3",
            JobPriority::Enterprise,
            ts3,
            spec_id,
        );

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

        let claimed: Option<JsonClaimedJob> = resp.json().await.expect("Failed to parse");
        let claimed = claimed.expect("Expected to claim a job");
        assert_eq!(
            claimed.uuid, first_job,
            "Expected first created job to be claimed first"
        );

        // Complete the job
        set_job_status(&server, first_job, JobStatus::Running);
        set_job_status(&server, first_job, JobStatus::Completed);

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

        let claimed: Option<JsonClaimedJob> = resp.json().await.expect("Failed to parse");
        let claimed = claimed.expect("Expected to claim a job");
        assert_eq!(
            claimed.uuid, second_job,
            "Expected second created job to be claimed second"
        );

        // Complete second job
        set_job_status(&server, second_job, JobStatus::Running);
        set_job_status(&server, second_job, JobStatus::Completed);

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

        let claimed: Option<JsonClaimedJob> = resp.json().await.expect("Failed to parse");
        let claimed = claimed.expect("Expected to claim a job");
        assert_eq!(
            claimed.uuid, third_job,
            "Expected third created job to be claimed third"
        );
    }

    // Test Free tier with different organizations can run concurrently
    #[tokio::test]
    async fn free_tier_different_orgs() {
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

        let (_, spec_id) = insert_test_spec(&server);
        let runner = create_runner(&server, &admin.token, "Free Diff Org Runner").await;
        let runner_token: &str = runner.token.as_ref();
        let runner_id = get_runner_id(&server, runner.uuid);
        associate_runner_spec(&server, runner_id, spec_id);

        let project1_id = get_project_id(&server, project1.slug.as_ref());
        let org1_id = get_organization_id(&server, project1_id);
        let report1_id = create_test_report(&server, project1_id);

        let project2_id = get_project_id(&server, project2.slug.as_ref());
        let org2_id = get_organization_id(&server, project2_id);
        let report2_id = create_test_report(&server, project2_id);

        // Insert Free tier jobs for different orgs
        let job1 = insert_test_job_full(
            &server,
            report1_id,
            project1.uuid,
            org1_id,
            "10.0.0.1",
            JobPriority::Free,
            spec_id,
        );
        let job2 = insert_test_job_full(
            &server,
            report2_id,
            project2.uuid,
            org2_id,
            "10.0.0.2",
            JobPriority::Free,
            spec_id,
        );

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

        let claimed: Option<JsonClaimedJob> = resp.json().await.expect("Failed to parse");
        let first_claimed = claimed.expect("Expected to claim first job");

        // Mark job as running
        set_job_status(&server, first_claimed.uuid, JobStatus::Running);

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

        let claimed: Option<JsonClaimedJob> = resp.json().await.expect("Failed to parse");
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
    async fn job_completion_unblocks() {
        let server = TestServer::new().await;
        let admin = server.signup("Admin", "unblock1@example.com").await;
        let org = server.create_org(&admin, "Unblock Org").await;
        let project = server.create_project(&admin, &org, "Unblock Project").await;

        let (_, spec_id) = insert_test_spec(&server);
        let runner = create_runner(&server, &admin.token, "Unblock Runner").await;
        let runner_token: &str = runner.token.as_ref();
        let runner_id = get_runner_id(&server, runner.uuid);
        associate_runner_spec(&server, runner_id, spec_id);

        let project_id = get_project_id(&server, project.slug.as_ref());
        let org_id = get_organization_id(&server, project_id);
        let report_id = create_test_report(&server, project_id);

        // Insert two Free tier jobs for the same org
        let job1 = insert_test_job_full(
            &server,
            report_id,
            project.uuid,
            org_id,
            "10.0.0.1",
            JobPriority::Free,
            spec_id,
        );
        let job2 = insert_test_job_full(
            &server,
            report_id,
            project.uuid,
            org_id,
            "10.0.0.2",
            JobPriority::Free,
            spec_id,
        );

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

        let claimed: Option<JsonClaimedJob> = resp.json().await.expect("Failed to parse");
        let first_claimed = claimed.expect("Expected to claim first job");

        set_job_status(&server, first_claimed.uuid, JobStatus::Running);

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

        let claimed: Option<JsonClaimedJob> = resp.json().await.expect("Failed to parse");
        assert!(
            claimed.is_none(),
            "Second job should be blocked while first is running"
        );

        // Complete the first job
        set_job_status(&server, first_claimed.uuid, JobStatus::Completed);

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

        let claimed: Option<JsonClaimedJob> = resp.json().await.expect("Failed to parse");
        let second_claimed = claimed.expect("Second job should be claimable after first completes");

        // Verify we got the second job (not the completed first one)
        let expected_second = if first_claimed.uuid == job1 {
            job2
        } else {
            job1
        };
        assert_eq!(
            second_claimed.uuid, expected_second,
            "Expected second job to be claimed after first completes"
        );
    }

    // Test that Free tier is blocked while Team tier is not for the same org
    #[tokio::test]
    async fn free_blocked_team_unblocked() {
        let server = TestServer::new().await;
        let admin = server.signup("Admin", "boundary1@example.com").await;
        let org = server.create_org(&admin, "Boundary Org").await;
        let project = server
            .create_project(&admin, &org, "Boundary Project")
            .await;

        let (_, spec_id) = insert_test_spec(&server);
        let runner = create_runner(&server, &admin.token, "Boundary Runner").await;
        let runner_token: &str = runner.token.as_ref();
        let runner_id = get_runner_id(&server, runner.uuid);
        associate_runner_spec(&server, runner_id, spec_id);

        let project_id = get_project_id(&server, project.slug.as_ref());
        let org_id = get_organization_id(&server, project_id);
        let report_id = create_test_report(&server, project_id);

        // Insert a Free tier job and mark it as running to block the org for Free tier
        let blocking_job = insert_test_job_full(
            &server,
            report_id,
            project.uuid,
            org_id,
            "10.0.0.1",
            JobPriority::Free,
            spec_id,
        );

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

        set_job_status(&server, blocking_job, JobStatus::Running);

        // Insert another Free tier job (should be blocked by org concurrency)
        let _free_blocked = insert_test_job_full(
            &server,
            report_id,
            project.uuid,
            org_id,
            "10.0.0.2",
            JobPriority::Free,
            spec_id,
        );

        // Insert a Team tier job (should NOT be blocked)
        let team_job = insert_test_job_full(
            &server,
            report_id,
            project.uuid,
            org_id,
            "10.0.0.3",
            JobPriority::Team,
            spec_id,
        );

        // Try to claim - should get the Team tier job, not the blocked Free tier job
        let body = serde_json::json!({ "poll_timeout": 1 });
        let resp = server
            .client
            .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        let claimed: Option<JsonClaimedJob> = resp.json().await.expect("Failed to parse");
        let claimed = claimed.expect("Expected to claim Team tier job");
        assert_eq!(
            claimed.uuid, team_job,
            "Team tier should be unlimited, not blocked like Free tier"
        );
    }

    // Test Unclaimed tier blocked by IP while Free tier from different org succeeds
    #[tokio::test]
    async fn unclaimed_blocked_free_different_org_unblocked() {
        let server = TestServer::new().await;
        let admin = server.signup("Admin", "boundary2@example.com").await;
        let org = server.create_org(&admin, "Boundary UF Org").await;
        let project = server
            .create_project(&admin, &org, "Boundary UF Project")
            .await;

        let (_, spec_id) = insert_test_spec(&server);
        let runner = create_runner(&server, &admin.token, "Boundary UF Runner").await;
        let runner_token: &str = runner.token.as_ref();
        let runner_id = get_runner_id(&server, runner.uuid);
        associate_runner_spec(&server, runner_id, spec_id);

        let project_id = get_project_id(&server, project.slug.as_ref());
        let org_id = get_organization_id(&server, project_id);
        let report_id = create_test_report(&server, project_id);

        // Start an Unclaimed job with same source IP to block that IP
        let same_ip = "192.168.1.50";
        let blocking_job = insert_test_job_full(
            &server,
            report_id,
            project.uuid,
            org_id,
            same_ip,
            JobPriority::Unclaimed,
            spec_id,
        );

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

        set_job_status(&server, blocking_job, JobStatus::Running);

        // Insert Unclaimed job with same IP (should be blocked by IP)
        let _unclaimed_blocked = insert_test_job_full(
            &server,
            report_id,
            project.uuid,
            org_id,
            same_ip,
            JobPriority::Unclaimed,
            spec_id,
        );

        // Insert Free tier job with same IP and same org
        // Blocked by org concurrency (running Unclaimed job counts against org)
        let _free_same_org = insert_test_job_full(
            &server,
            report_id,
            project.uuid,
            org_id,
            same_ip,
            JobPriority::Free,
            spec_id,
        );

        // Insert a Free tier job with DIFFERENT IP and DIFFERENT org to prove Free tier works
        let org2 = server.create_org(&admin, "Boundary UF Org 2").await;
        let project2 = server
            .create_project(&admin, &org2, "Boundary UF Project 2")
            .await;
        let second_project_id = get_project_id(&server, project2.slug.as_ref());
        let second_org_id = get_organization_id(&server, second_project_id);
        let second_report_id = create_test_report(&server, second_project_id);

        let free_unblocked = insert_test_job_full(
            &server,
            second_report_id,
            project2.uuid,
            second_org_id,
            "10.0.0.99",
            JobPriority::Free,
            spec_id,
        );

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

        let claimed: Option<JsonClaimedJob> = resp.json().await.expect("Failed to parse");
        let claimed = claimed.expect("Expected to claim unblocked Free tier job");
        assert_eq!(
            claimed.uuid, free_unblocked,
            "Free tier from different org should be claimable"
        );
    }

    // Test that jobs with identical timestamps are ordered deterministically by id
    #[tokio::test]
    async fn fifo_same_timestamp_tiebreaker() {
        use common::insert_test_job_with_timestamp;

        let server = TestServer::new().await;
        let admin = server.signup("Admin", "fifo-tie@example.com").await;
        let org = server.create_org(&admin, "FIFO Tie Org").await;
        let project = server
            .create_project(&admin, &org, "FIFO Tie Project")
            .await;

        let (_, spec_id) = insert_test_spec(&server);
        let runner = create_runner(&server, &admin.token, "FIFO Tie Runner").await;
        let runner_token: &str = runner.token.as_ref();
        let runner_id = get_runner_id(&server, runner.uuid);
        associate_runner_spec(&server, runner_id, spec_id);

        let project_id = get_project_id(&server, project.slug.as_ref());
        let org_id = get_organization_id(&server, project_id);
        let report_id = create_test_report(&server, project_id);

        // Use a single fixed timestamp for all jobs so the `created` column is identical.
        let fixed_ts = DateTime::now();

        // Insert 3 Enterprise-tier jobs with the exact same timestamp.
        // They will get sequential database IDs.
        let first_job = insert_test_job_with_timestamp(
            &server,
            report_id,
            project.uuid,
            org_id,
            "10.0.0.1",
            JobPriority::Enterprise,
            fixed_ts,
            spec_id,
        );
        let second_job = insert_test_job_with_timestamp(
            &server,
            report_id,
            project.uuid,
            org_id,
            "10.0.0.2",
            JobPriority::Enterprise,
            fixed_ts,
            spec_id,
        );
        let third_job = insert_test_job_with_timestamp(
            &server,
            report_id,
            project.uuid,
            org_id,
            "10.0.0.3",
            JobPriority::Enterprise,
            fixed_ts,
            spec_id,
        );

        // Claim first - should be first_job (lowest id)
        let body = serde_json::json!({ "poll_timeout": 1 });
        let resp = server
            .client
            .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&body)
            .send()
            .await
            .expect("Request failed");

        let claimed: Option<JsonClaimedJob> = resp.json().await.expect("Failed to parse");
        let claimed = claimed.expect("Expected to claim a job");
        assert_eq!(
            claimed.uuid, first_job,
            "Expected first inserted job (lowest id) to be claimed first"
        );

        // Complete the job
        set_job_status(&server, first_job, JobStatus::Running);
        set_job_status(&server, first_job, JobStatus::Completed);

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

        let claimed: Option<JsonClaimedJob> = resp.json().await.expect("Failed to parse");
        let claimed = claimed.expect("Expected to claim a job");
        assert_eq!(
            claimed.uuid, second_job,
            "Expected second inserted job to be claimed second"
        );

        // Complete the second job
        set_job_status(&server, second_job, JobStatus::Running);
        set_job_status(&server, second_job, JobStatus::Completed);

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

        let claimed: Option<JsonClaimedJob> = resp.json().await.expect("Failed to parse");
        let claimed = claimed.expect("Expected to claim a job");
        assert_eq!(
            claimed.uuid, third_job,
            "Expected third inserted job to be claimed last"
        );
    }
}

// =============================================================================
// Poll Timeout Boundary Tests
// =============================================================================

mod poll_timeout_boundaries {
    use super::*;

    // poll_timeout: 0 is below PollTimeout::MIN (1), so the server rejects it
    // at deserialization time with 400 Bad Request.
    #[tokio::test]
    async fn poll_timeout_zero_clamps_to_min() {
        let server = TestServer::new().await;
        let admin = server.signup("Admin", "poll-zero@example.com").await;

        let (_, spec_id) = insert_test_spec(&server);
        let runner = create_runner(&server, &admin.token, "Poll Zero Runner").await;
        let runner_token: &str = runner.token.as_ref();
        let runner_id = get_runner_id(&server, runner.uuid);
        associate_runner_spec(&server, runner_id, spec_id);

        let resp = server
            .client
            .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
            .header("Authorization", format!("Bearer {runner_token}"))
            .json(&serde_json::json!({ "poll_timeout": 0 }))
            .send()
            .await
            .expect("Request failed");

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    // poll_timeout: 61 with a job available should return immediately (clamped to 60)
    #[tokio::test]
    async fn poll_timeout_exceeds_max() {
        let server = TestServer::new().await;
        let admin = server.signup("Admin", "poll-max@example.com").await;
        let org = server.create_org(&admin, "Poll Max Org").await;
        let project = server
            .create_project(&admin, &org, "Poll Max Project")
            .await;

        let (_, spec_id) = insert_test_spec(&server);
        let runner = create_runner(&server, &admin.token, "Poll Max Runner").await;
        let runner_token: &str = runner.token.as_ref();
        let runner_id = get_runner_id(&server, runner.uuid);
        associate_runner_spec(&server, runner_id, spec_id);

        let project_id = get_project_id(&server, project.slug.as_ref());
        let report_id = create_test_report(&server, project_id);
        let _job_uuid = insert_test_job(&server, report_id, spec_id);

        let body = serde_json::json!({ "poll_timeout": 61 });

        // With paused time, any tokio::time::sleep blocks forever unless time is advanced.
        // If the task completes without advancing, the endpoint returned immediately.
        tokio::time::pause();
        let handle = tokio::spawn({
            let client = server.client.clone();
            let url = server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid));
            let token = runner_token.to_owned();
            async move {
                client
                    .post(url)
                    .header("Authorization", format!("Bearer {token}"))
                    .json(&body)
                    .send()
                    .await
                    .expect("Request failed")
            }
        });

        let resp = handle.await.expect("Task panicked");
        assert_eq!(resp.status(), StatusCode::OK);
        let claimed: Option<JsonClaimedJob> = resp.json().await.expect("Failed to parse");
        assert!(claimed.is_some(), "Expected to claim a job");
    }
}

// =============================================================================
// Concurrency Safety Tests
// =============================================================================
//
// These tests validate that the TOCTOU fix prevents concurrent runners from
// bypassing tier-based concurrency limits.

mod concurrency_safety {
    use super::*;
    use bencher_json::JobPriority;
    use common::{get_organization_id, insert_test_job_full};

    // Two runners race to claim Free-tier jobs for the same org.
    // Only one should claim because of the 1-per-org concurrency limit.
    // This validates the TOCTOU fix: read+check+update under a single write lock.
    #[tokio::test]
    async fn concurrent_free_tier_claim_respects_org_limit() {
        let server = TestServer::new().await;
        let admin = server.signup("Admin", "concurrent-free@example.com").await;
        let org = server.create_org(&admin, "Concurrent Free Org").await;
        let project = server
            .create_project(&admin, &org, "Concurrent Free Project")
            .await;

        let (_, spec_id) = insert_test_spec(&server);
        let runner1 = create_runner(&server, &admin.token, "Conc Free Runner 1").await;
        let runner1_token: String = runner1.token.as_ref().to_owned();
        let runner1_id = get_runner_id(&server, runner1.uuid);
        associate_runner_spec(&server, runner1_id, spec_id);
        let runner2 = create_runner(&server, &admin.token, "Conc Free Runner 2").await;
        let runner2_token: String = runner2.token.as_ref().to_owned();
        let runner2_id = get_runner_id(&server, runner2.uuid);
        associate_runner_spec(&server, runner2_id, spec_id);

        let project_id = get_project_id(&server, project.slug.as_ref());
        let org_id = get_organization_id(&server, project_id);
        let report_id = create_test_report(&server, project_id);

        // Insert two Free tier jobs for the same org
        let _job1 = insert_test_job_full(
            &server,
            report_id,
            project.uuid,
            org_id,
            "10.0.0.1",
            JobPriority::Free,
            spec_id,
        );
        let _job2 = insert_test_job_full(
            &server,
            report_id,
            project.uuid,
            org_id,
            "10.0.0.2",
            JobPriority::Free,
            spec_id,
        );

        let claim_body = serde_json::json!({ "poll_timeout": 1 });
        let url1 = server.api_url(&format!("/v0/runners/{}/jobs", runner1.uuid));
        let url2 = server.api_url(&format!("/v0/runners/{}/jobs", runner2.uuid));
        let client = &server.client;

        // Race both runners to claim simultaneously
        let (resp1, resp2) = (
            async {
                client
                    .post(&url1)
                    .header("Authorization", format!("Bearer {runner1_token}"))
                    .json(&claim_body)
                    .send()
                    .await
                    .expect("Request 1 failed")
            },
            async {
                client
                    .post(&url2)
                    .header("Authorization", format!("Bearer {runner2_token}"))
                    .json(&claim_body)
                    .send()
                    .await
                    .expect("Request 2 failed")
            },
        )
            .join()
            .await;

        assert_eq!(resp1.status(), StatusCode::OK);
        assert_eq!(resp2.status(), StatusCode::OK);

        let job1: Option<JsonClaimedJob> = resp1.json().await.expect("Failed to parse response 1");
        let job2: Option<JsonClaimedJob> = resp2.json().await.expect("Failed to parse response 2");

        let claimed_count = [&job1, &job2].iter().filter(|j| j.is_some()).count();

        // Free tier: at most 1 concurrent job per org.
        // After the first claim, the second runner should see the org as blocked.
        assert_eq!(
            claimed_count, 1,
            "Free tier allows at most 1 concurrent job per org"
        );
    }

    // Two runners race to claim Unclaimed-tier jobs for the same source IP.
    // The concurrency limit should prevent both from claiming simultaneously.
    #[tokio::test]
    async fn concurrent_unclaimed_tier_claim_respects_ip_limit() {
        let server = TestServer::new().await;
        let admin = server
            .signup("Admin", "concurrent-unclaimed@example.com")
            .await;
        let org = server.create_org(&admin, "Concurrent Unclaimed Org").await;
        let project = server
            .create_project(&admin, &org, "Concurrent Unclaimed Project")
            .await;

        let (_, spec_id) = insert_test_spec(&server);
        let runner1 = create_runner(&server, &admin.token, "Conc Uncl Runner 1").await;
        let runner1_token: String = runner1.token.as_ref().to_owned();
        let runner1_id = get_runner_id(&server, runner1.uuid);
        associate_runner_spec(&server, runner1_id, spec_id);
        let runner2 = create_runner(&server, &admin.token, "Conc Uncl Runner 2").await;
        let runner2_token: String = runner2.token.as_ref().to_owned();
        let runner2_id = get_runner_id(&server, runner2.uuid);
        associate_runner_spec(&server, runner2_id, spec_id);

        let project_id = get_project_id(&server, project.slug.as_ref());
        let org_id = get_organization_id(&server, project_id);
        let report_id = create_test_report(&server, project_id);

        // Insert two Unclaimed tier jobs with the same source IP
        let same_ip = "192.168.1.200";
        let _job1 = insert_test_job_full(
            &server,
            report_id,
            project.uuid,
            org_id,
            same_ip,
            JobPriority::Unclaimed,
            spec_id,
        );
        let _job2 = insert_test_job_full(
            &server,
            report_id,
            project.uuid,
            org_id,
            same_ip,
            JobPriority::Unclaimed,
            spec_id,
        );

        let claim_body = serde_json::json!({ "poll_timeout": 1 });
        let url1 = server.api_url(&format!("/v0/runners/{}/jobs", runner1.uuid));
        let url2 = server.api_url(&format!("/v0/runners/{}/jobs", runner2.uuid));
        let client = &server.client;

        // Race both runners to claim simultaneously
        let (resp1, resp2) = (
            async {
                client
                    .post(&url1)
                    .header("Authorization", format!("Bearer {runner1_token}"))
                    .json(&claim_body)
                    .send()
                    .await
                    .expect("Request 1 failed")
            },
            async {
                client
                    .post(&url2)
                    .header("Authorization", format!("Bearer {runner2_token}"))
                    .json(&claim_body)
                    .send()
                    .await
                    .expect("Request 2 failed")
            },
        )
            .join()
            .await;

        assert_eq!(resp1.status(), StatusCode::OK);
        assert_eq!(resp2.status(), StatusCode::OK);

        let job1: Option<JsonClaimedJob> = resp1.json().await.expect("Failed to parse response 1");
        let job2: Option<JsonClaimedJob> = resp2.json().await.expect("Failed to parse response 2");

        let claimed_count = [&job1, &job2].iter().filter(|j| j.is_some()).count();

        // Unclaimed tier: at most 1 concurrent job per source IP.
        // After the first claim, the second runner should see the IP as blocked.
        assert_eq!(
            claimed_count, 1,
            "Unclaimed tier allows at most 1 concurrent job per source IP"
        );
    }
}

// =============================================================================
// Auth Cross-validation Tests
// =============================================================================

// A valid user JWT token should be rejected on runner endpoints
#[tokio::test]
async fn user_jwt_rejected_on_runner_endpoint() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "userjwt@example.com").await;

    let runner = create_runner(&server, &admin.token, "JWT Test Runner").await;

    let body = serde_json::json!({ "poll_timeout": 1 });

    // Use the user's JWT (not a runner token) on the runner claim endpoint
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "User JWT should be rejected on runner endpoints"
    );
}

// =============================================================================
// Multi-Spec Claiming Tests
// =============================================================================

/// A runner associated with multiple specs can claim jobs requiring any of them.
/// Uses Team priority to avoid per-IP concurrency limits on the unclaimed tier.
#[tokio::test]
async fn runner_multiple_specs_claims_matching_jobs() {
    use bencher_json::JobPriority;
    use common::insert_test_job_full;

    let server = TestServer::new().await;
    let admin = server.signup("Admin", "multispec@example.com").await;
    let org = server.create_org(&admin, "Multi Spec Org").await;
    let project = server
        .create_project(&admin, &org, "Multi Spec Project")
        .await;

    // Create two different specs
    let (_, spec_x86_id) = insert_test_spec_full(
        &server,
        "x86_64",
        2,
        0x0001_0000_0000,
        10_737_418_240,
        false,
    );
    let (_, spec_arm_id) = insert_test_spec_full(
        &server,
        "aarch64",
        4,
        0x0002_0000_0000,
        21_474_836_480,
        true,
    );

    // Create a runner and associate it with both specs
    let runner = create_runner(&server, &admin.token, "Multi Spec Runner").await;
    let runner_token: &str = runner.token.as_ref();
    let runner_id = get_runner_id(&server, runner.uuid);
    associate_runner_spec(&server, runner_id, spec_x86_id);
    associate_runner_spec(&server, runner_id, spec_arm_id);

    // Insert jobs with Team priority (unlimited concurrency) to avoid IP limits
    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let org_id = common::get_organization_id(&server, project_id);
    let job_a = insert_test_job_full(
        &server,
        report_id,
        project.uuid,
        org_id,
        common::TEST_SOURCE_IP,
        JobPriority::Team,
        spec_x86_id,
    );
    let job_b = insert_test_job_full(
        &server,
        report_id,
        project.uuid,
        org_id,
        common::TEST_SOURCE_IP,
        JobPriority::Team,
        spec_arm_id,
    );

    // First claim should get one job
    let claim_body = serde_json::json!({ "poll_timeout": 1 });
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
        .header("Authorization", format!("Bearer {runner_token}"))
        .json(&claim_body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let first: Option<JsonClaimedJob> = resp.json().await.expect("Failed to parse response");
    let first = first.expect("Expected to claim first job");

    // Second claim should get the other job
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
        .header("Authorization", format!("Bearer {runner_token}"))
        .json(&claim_body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let second: Option<JsonClaimedJob> = resp.json().await.expect("Failed to parse response");
    let second = second.expect("Expected to claim second job");

    // Both jobs should have been claimed (order depends on priority/FIFO)
    let claimed_uuids = [first.uuid, second.uuid];
    assert!(
        claimed_uuids.contains(&job_a),
        "Job A should have been claimed"
    );
    assert!(
        claimed_uuids.contains(&job_b),
        "Job B should have been claimed"
    );
    assert_ne!(first.uuid, second.uuid, "Should claim two different jobs");
}

// =============================================================================
// Auth-Boundary Tests (Fix 16)
// =============================================================================

/// Non-admin user cannot PATCH a runner.
#[tokio::test]
async fn non_admin_cannot_patch_runner() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "auth-patch-admin@example.com").await;
    let user = server.signup("User", "auth-patch-user@example.com").await;

    let runner = create_runner(&server, &admin.token, "Patch Auth Runner").await;

    let body = serde_json::json!({ "name": "Renamed Runner" });
    let resp = server
        .client
        .patch(server.api_url(&format!("/v0/runners/{}", runner.uuid)))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "Non-admin should not be able to PATCH a runner"
    );
}

/// Wrong runner's token cannot claim a job assigned to a different runner.
#[tokio::test]
async fn wrong_runner_token_claim() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "auth-wrong-claim@example.com").await;

    let runner1 = create_runner(&server, &admin.token, "Auth Runner One").await;
    let runner2 = create_runner(&server, &admin.token, "Auth Runner Two").await;
    let runner2_token: &str = runner2.token.as_ref();

    // Use runner2's token to claim on runner1's endpoint
    let body = serde_json::json!({ "poll_timeout": 1 });
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/jobs", runner1.uuid)))
        .header("Authorization", format!("Bearer {runner2_token}"))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    // Token hash won't match runner1 — should fail auth
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

/// Archived runner's token cannot claim jobs.
#[tokio::test]
async fn archived_runner_token_rejected() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "auth-archived@example.com").await;

    let (_, spec_id) = insert_test_spec(&server);
    let runner = create_runner(&server, &admin.token, "Archived Auth Runner").await;
    let runner_token: &str = runner.token.as_ref();
    let runner_id = get_runner_id(&server, runner.uuid);
    associate_runner_spec(&server, runner_id, spec_id);

    // Archive the runner
    let body = serde_json::json!({ "archived": true });
    server
        .client
        .patch(server.api_url(&format!("/v0/runners/{}", runner.uuid)))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    // Try to claim with the archived runner's token
    let body = serde_json::json!({ "poll_timeout": 1 });
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
        .header("Authorization", format!("Bearer {runner_token}"))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "Archived runner's token should be rejected"
    );
}

// =============================================================================
// Additional Concurrency Tier Tests (Fix 2)
// =============================================================================

/// Free tier: blocked when same org has in-flight, even from a different runner.
#[tokio::test]
async fn free_tier_blocked_same_org_different_runner() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "free-diff-runner@example.com").await;
    let org = server.create_org(&admin, "Free DiffRunner Org").await;
    let project = server
        .create_project(&admin, &org, "Free DiffRunner Proj")
        .await;

    let (_, spec_id) = insert_test_spec(&server);
    let runner1 = create_runner(&server, &admin.token, "FreeOrgRunner1").await;
    let runner1_token: &str = runner1.token.as_ref();
    let runner1_id = get_runner_id(&server, runner1.uuid);
    associate_runner_spec(&server, runner1_id, spec_id);
    let runner2 = create_runner(&server, &admin.token, "FreeOrgRunner2").await;
    let runner2_token: &str = runner2.token.as_ref();
    let runner2_id = get_runner_id(&server, runner2.uuid);
    associate_runner_spec(&server, runner2_id, spec_id);

    let project_id = get_project_id(&server, project.slug.as_ref());
    let org_id = common::get_organization_id(&server, project_id);
    let report_id = create_test_report(&server, project_id);

    // Two Free tier jobs for the same org
    let job1 = insert_test_job_full(
        &server,
        report_id,
        project.uuid,
        org_id,
        "10.0.0.1",
        JobPriority::Free,
        spec_id,
    );
    let _job2 = insert_test_job_full(
        &server,
        report_id,
        project.uuid,
        org_id,
        "10.0.0.2",
        JobPriority::Free,
        spec_id,
    );

    // Runner1 claims first job
    let body = serde_json::json!({ "poll_timeout": 1 });
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/jobs", runner1.uuid)))
        .header("Authorization", format!("Bearer {runner1_token}"))
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    let claimed: Option<JsonClaimedJob> = resp.json().await.expect("Failed to parse");
    assert_eq!(claimed.as_ref().map(|j| j.uuid), Some(job1));

    // Set to running
    set_job_status(&server, job1, JobStatus::Running);
    set_job_runner_id(&server, job1, runner1_id);

    // Runner2 tries to claim second job — same org, should be blocked
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/jobs", runner2.uuid)))
        .header("Authorization", format!("Bearer {runner2_token}"))
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    let claimed: Option<JsonClaimedJob> = resp.json().await.expect("Failed to parse");
    assert!(
        claimed.is_none(),
        "Free tier: second job should be blocked for same org"
    );
}

/// Enterprise/Team tier: mixed scenario where free is blocked but enterprise is claimable.
#[tokio::test]
async fn mixed_tier_free_blocked_enterprise_claimable() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "mix-tier@example.com").await;
    let org = server.create_org(&admin, "MixTier Org").await;
    let project = server.create_project(&admin, &org, "MixTier Project").await;

    let (_, spec_id) = insert_test_spec(&server);
    let runner = create_runner(&server, &admin.token, "MixTier Runner").await;
    let runner_token: &str = runner.token.as_ref();
    let runner_id = get_runner_id(&server, runner.uuid);
    associate_runner_spec(&server, runner_id, spec_id);

    let project_id = get_project_id(&server, project.slug.as_ref());
    let org_id = common::get_organization_id(&server, project_id);
    let report_id = create_test_report(&server, project_id);

    // Insert a Free tier job and mark it as running to block the org
    let blocking_job = insert_test_job_full(
        &server,
        report_id,
        project.uuid,
        org_id,
        "10.0.0.1",
        JobPriority::Free,
        spec_id,
    );
    set_job_status(&server, blocking_job, JobStatus::Running);
    set_job_runner_id(&server, blocking_job, runner_id);

    // Insert a Free tier job (should be blocked) and an Enterprise tier job (should be claimable)
    let _free_job = insert_test_job_full(
        &server,
        report_id,
        project.uuid,
        org_id,
        "10.0.0.2",
        JobPriority::Free,
        spec_id,
    );
    let enterprise_job = insert_test_job_full(
        &server,
        report_id,
        project.uuid,
        org_id,
        "10.0.0.3",
        JobPriority::Enterprise,
        spec_id,
    );

    // Claim should skip the blocked Free job and grab the Enterprise job
    let body = serde_json::json!({ "poll_timeout": 1 });
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
        .header("Authorization", format!("Bearer {runner_token}"))
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    let claimed: Option<JsonClaimedJob> = resp.json().await.expect("Failed to parse");
    assert_eq!(
        claimed.as_ref().map(|j| j.uuid),
        Some(enterprise_job),
        "Enterprise job should be claimable even when Free tier is blocked"
    );
}

// Claiming an organization upgrades pending Unclaimed jobs to Free priority.
#[tokio::test]
#[expect(clippy::too_many_lines)]
async fn claim_upgrades_pending_job_priority() {
    use bencher_json::{DateTime, JobStatus, OrganizationSlug, OrganizationUuid, ProjectUuid};
    use bencher_schema::schema;
    use common::{get_organization_id, insert_test_job_full};
    use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};

    let server = TestServer::new().await;
    let admin = server.signup("Admin", "claim-upgrade@example.com").await;

    // --- Insert an unclaimed organization directly (no members) ---
    let unclaimed_org_uuid = OrganizationUuid::new();
    let unclaimed_org_slug: OrganizationSlug = format!("unclaimed-{unclaimed_org_uuid}")
        .parse()
        .expect("Invalid slug");
    let now = DateTime::now();
    {
        let mut conn = server.db_conn();
        diesel::insert_into(schema::organization::table)
            .values((
                schema::organization::uuid.eq(&unclaimed_org_uuid),
                schema::organization::name.eq("Unclaimed Upgrade Org"),
                schema::organization::slug.eq(&unclaimed_org_slug),
                schema::organization::created.eq(&now),
                schema::organization::modified.eq(&now),
            ))
            .execute(&mut conn)
            .expect("Failed to insert unclaimed org");
    }

    // Get the org's internal id
    let unclaimed_org_id: i32 = {
        let mut conn = server.db_conn();
        schema::organization::table
            .filter(schema::organization::uuid.eq(&unclaimed_org_uuid))
            .select(schema::organization::id)
            .first(&mut conn)
            .expect("Failed to get unclaimed org ID")
    };

    // --- Insert a project for the unclaimed org ---
    let project_uuid = ProjectUuid::new();
    let project_slug = format!("proj-{project_uuid}");
    {
        let mut conn = server.db_conn();
        diesel::insert_into(schema::project::table)
            .values((
                schema::project::uuid.eq(&project_uuid),
                schema::project::organization_id.eq(unclaimed_org_id),
                schema::project::name.eq("Upgrade Test Project"),
                schema::project::slug.eq(&project_slug),
                schema::project::visibility.eq(0),
                schema::project::created.eq(&now),
                schema::project::modified.eq(&now),
            ))
            .execute(&mut conn)
            .expect("Failed to insert project");
    }
    let project_id = get_project_id(&server, &project_slug);
    let report_id = create_test_report(&server, project_id);
    let (_, spec_id) = insert_test_spec(&server);

    // --- Insert pending Unclaimed jobs for the unclaimed org ---
    let pending_job_1 = insert_test_job_full(
        &server,
        report_id,
        project_uuid,
        unclaimed_org_id,
        "10.0.0.1",
        JobPriority::Unclaimed,
        spec_id,
    );
    let pending_job_2 = insert_test_job_full(
        &server,
        report_id,
        project_uuid,
        unclaimed_org_id,
        "10.0.0.2",
        JobPriority::Unclaimed,
        spec_id,
    );

    // --- Insert a Running Unclaimed job (should NOT be upgraded) ---
    let running_job = insert_test_job_full(
        &server,
        report_id,
        project_uuid,
        unclaimed_org_id,
        "10.0.0.3",
        JobPriority::Unclaimed,
        spec_id,
    );
    set_job_status(&server, running_job, JobStatus::Running);

    // --- Insert a pending job for a DIFFERENT org (should NOT be upgraded) ---
    let other_org = server.create_org(&admin, "Other Org Upgrade").await;
    let other_project = server
        .create_project(&admin, &other_org, "Other Upgrade Proj")
        .await;
    let other_project_id = get_project_id(&server, other_project.slug.as_ref());
    let other_org_id = get_organization_id(&server, other_project_id);
    let other_report_id = create_test_report(&server, other_project_id);
    let other_job = insert_test_job_full(
        &server,
        other_report_id,
        other_project.uuid,
        other_org_id,
        "10.0.0.4",
        JobPriority::Unclaimed,
        spec_id,
    );

    // --- Claim the unclaimed organization ---
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/organizations/{unclaimed_org_slug}/claim")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&serde_json::json!({}))
        .send()
        .await
        .expect("Claim request failed");
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "Claim should succeed: {}",
        resp.text().await.unwrap_or_default()
    );

    // --- Assert: pending jobs for the claimed org now have Free priority ---
    assert_eq!(
        get_job_priority(&server, pending_job_1),
        JobPriority::Free,
        "Pending job 1 should be upgraded to Free"
    );
    assert_eq!(
        get_job_priority(&server, pending_job_2),
        JobPriority::Free,
        "Pending job 2 should be upgraded to Free"
    );

    // --- Assert: Running job still has Unclaimed priority ---
    assert_eq!(
        get_job_priority(&server, running_job),
        JobPriority::Unclaimed,
        "Running job should NOT be upgraded"
    );

    // --- Assert: other org's job still has Unclaimed priority ---
    assert_eq!(
        get_job_priority(&server, other_job),
        JobPriority::Unclaimed,
        "Other org's job should NOT be upgraded"
    );
}

// Claiming an organization with no pending jobs succeeds without error.
#[tokio::test]
async fn claim_no_pending_jobs_succeeds() {
    use bencher_json::{DateTime, OrganizationSlug, OrganizationUuid};
    use bencher_schema::schema;
    use diesel::{ExpressionMethods as _, RunQueryDsl as _};

    let server = TestServer::new().await;
    let admin = server.signup("Admin", "claim-nojobs@example.com").await;

    // Insert an unclaimed organization directly (no members, no jobs)
    let unclaimed_org_uuid = OrganizationUuid::new();
    let unclaimed_org_slug: OrganizationSlug = format!("unclaimed-nj-{unclaimed_org_uuid}")
        .parse()
        .expect("Invalid slug");
    let now = DateTime::now();
    {
        let mut conn = server.db_conn();
        diesel::insert_into(schema::organization::table)
            .values((
                schema::organization::uuid.eq(&unclaimed_org_uuid),
                schema::organization::name.eq("Unclaimed No Jobs Org"),
                schema::organization::slug.eq(&unclaimed_org_slug),
                schema::organization::created.eq(&now),
                schema::organization::modified.eq(&now),
            ))
            .execute(&mut conn)
            .expect("Failed to insert unclaimed org");
    }

    // Claim should succeed even with no pending jobs
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/organizations/{unclaimed_org_slug}/claim")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&serde_json::json!({}))
        .send()
        .await
        .expect("Claim request failed");
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "Claim with no pending jobs should succeed: {}",
        resp.text().await.unwrap_or_default()
    );
}

// =============================================================================
// Recovery Tests
// =============================================================================

/// Test that `recover_orphaned_claimed_jobs` marks orphaned Claimed jobs as Failed.
/// A job is "orphaned" when it's been in Claimed state longer than the heartbeat timeout.
#[tokio::test]
async fn recover_orphaned_claimed_jobs_marks_as_failed() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "recover-orphan@example.com").await;
    let org = server.create_org(&admin, "Recover Orphan").await;
    let project = server
        .create_project(&admin, &org, "Recover Orphan proj")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let (_, spec_id) = insert_test_spec(&server);
    let job_uuid = insert_test_job(&server, report_id, spec_id);

    // Set job to Claimed with an old claimed timestamp (5 minutes ago)
    let old_timestamp: i64 = DateTime::now().timestamp() - 300;
    let old_time: DateTime = old_timestamp.try_into().expect("Invalid timestamp");
    {
        let mut conn = server.db_conn();
        diesel::update(schema::job::table.filter(schema::job::uuid.eq(job_uuid)))
            .set((
                schema::job::status.eq(JobStatus::Claimed),
                schema::job::claimed.eq(Some(old_time)),
                schema::job::modified.eq(old_time),
            ))
            .execute(&mut conn)
            .expect("Failed to set job to claimed");
    }

    // Call recover_orphaned_claimed_jobs with a 5-second timeout
    let log = slog::Logger::root(slog::Discard, slog::o!());
    let heartbeat_timeout = std::time::Duration::from_secs(5);
    let mut conn = server.db_conn();
    let recovered = recover_orphaned_claimed_jobs(
        &log,
        &mut conn,
        heartbeat_timeout,
        &bencher_json::Clock::System,
    );
    assert_eq!(recovered, 1, "Expected 1 orphaned job to be recovered");

    // Verify job is now Failed
    let status: JobStatus = schema::job::table
        .filter(schema::job::uuid.eq(job_uuid))
        .select(schema::job::status)
        .first(&mut conn)
        .expect("Failed to get job status");
    assert_eq!(
        status,
        JobStatus::Failed,
        "Orphaned claimed job should be Failed"
    );
}

/// Test that `spawn_heartbeat_timeout` marks a Running job as Failed after timeout.
/// Simulates what `spawn_job_recovery` does for in-flight jobs on server restart.
///
/// Uses `tokio::time::pause()` / `advance()` to make the test deterministic
/// without wall-clock waits. The `last_heartbeat` is cleared to `None` so
/// the freshness check inside `spawn_heartbeat_timeout` is skipped, and the
/// main `tokio::time::sleep(timeout)` respects virtual time.
#[tokio::test]
async fn spawn_heartbeat_timeout_fails_running_job() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "hb-running@example.com").await;
    let org = server.create_org(&admin, "Hb Running").await;
    let project = server.create_project(&admin, &org, "Hb Running proj").await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let (_, spec_id) = insert_test_spec(&server);
    let job_uuid = insert_test_job(&server, report_id, spec_id);

    // Set job to Running via direct DB update (last_heartbeat stays NULL)
    set_job_status(&server, job_uuid, JobStatus::Running);

    // Get the JobId for spawn_heartbeat_timeout
    let job_id: JobId = {
        let mut conn = server.db_conn();
        schema::job::table
            .filter(schema::job::uuid.eq(job_uuid))
            .select(schema::job::id)
            .first(&mut conn)
            .expect("Failed to get job ID")
    };

    // Create the infrastructure spawn_heartbeat_timeout needs
    let connection = Arc::new(Mutex::new(server.db_conn()));
    let heartbeat_tasks = HeartbeatTasks::new();
    let log = slog::Logger::root(slog::Discard, slog::o!());
    let timeout = std::time::Duration::from_secs(5);
    let grace_period = std::time::Duration::from_secs(60);

    // Pause time before spawning so the sleep timer is registered in virtual time
    tokio::time::pause();

    spawn_heartbeat_timeout(
        log,
        timeout,
        connection,
        job_id,
        &heartbeat_tasks,
        grace_period,
        bencher_json::Clock::System,
    );

    // Let the spawned task start and register its sleep(5s) timer
    tokio::task::yield_now().await;
    // Advance past the 5s timeout
    tokio::time::advance(std::time::Duration::from_secs(6)).await;
    // Let the spawned task complete after the timer fires
    tokio::task::yield_now().await;
    tokio::time::resume();

    // Verify the job is now Failed
    let mut conn = server.db_conn();
    let status: JobStatus = schema::job::table
        .filter(schema::job::uuid.eq(job_uuid))
        .select(schema::job::status)
        .first(&mut conn)
        .expect("Failed to get job status");
    assert_eq!(
        status,
        JobStatus::Failed,
        "Running job should be Failed after heartbeat timeout"
    );
}

/// Claiming a job with invalid config (missing required fields) fails gracefully.
/// The claim endpoint parses the config after claiming, so the job transitions to
/// Claimed but the response is an error.
#[tokio::test]
async fn claim_job_invalid_config() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "invalid-config@example.com").await;
    let org = server.create_org(&admin, "Invalid Config").await;
    let project = server
        .create_project(&admin, &org, "Invalid Config proj")
        .await;

    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let (_, spec_id) = insert_test_spec(&server);

    // Insert a job with invalid config (missing required fields like digest, timeout)
    insert_test_job_with_invalid_config(&server, report_id, spec_id);

    let runner = create_runner(&server, &admin.token, "Invalid Config Runner").await;
    let runner_token: &str = runner.token.as_ref();
    let runner_id = get_runner_id(&server, runner.uuid);
    associate_runner_spec(&server, runner_id, spec_id);

    // Try to claim the job — parse_config should fail, returning an error
    let body = serde_json::json!({ "poll_timeout": 1 });
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
        .header("Authorization", format!("Bearer {runner_token}"))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    // The claim should fail because the config cannot be parsed
    assert_ne!(
        resp.status(),
        StatusCode::OK,
        "Claiming a job with invalid config should not succeed"
    );
}

/// Heartbeat timeout for a claimed job without a WS channel.
/// Simulates what `spawn_job_recovery` does on server restart: spawns heartbeat
/// timeouts for in-flight jobs that were claimed but never opened a WS connection.
///
/// Uses `tokio::time::pause()` / `advance()` to make the test deterministic.
/// `last_heartbeat` is cleared to `None` so the freshness check is skipped,
/// and the main `tokio::time::sleep(timeout)` respects virtual time.
#[tokio::test]
async fn heartbeat_timeout_claimed_job_without_ws() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "hb-nows@example.com").await;
    let org = server.create_org(&admin, "Hb No WS").await;
    let project = server.create_project(&admin, &org, "Hb No WS proj").await;

    let (_, spec_id) = insert_test_spec(&server);
    let runner = create_runner(&server, &admin.token, "No WS Runner").await;
    let runner_token: &str = runner.token.as_ref();
    let runner_id = get_runner_id(&server, runner.uuid);
    associate_runner_spec(&server, runner_id, spec_id);

    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let job_uuid = insert_test_job(&server, report_id, spec_id);

    // Claim the job via the REST API (no WS connection opened).
    // This sets last_heartbeat to DateTime::now().
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
    let claimed: Option<JsonClaimedJob> = resp.json().await.expect("Failed to parse response");
    let claimed = claimed.expect("Expected to claim a job");
    assert_eq!(claimed.uuid, job_uuid);

    // Get the JobId for spawn_heartbeat_timeout
    let job_id: JobId = {
        let mut conn = server.db_conn();
        schema::job::table
            .filter(schema::job::uuid.eq(job_uuid))
            .select(schema::job::id)
            .first(&mut conn)
            .expect("Failed to get job ID")
    };

    // Clear last_heartbeat so the freshness check is skipped
    {
        let mut conn = server.db_conn();
        diesel::update(schema::job::table.filter(schema::job::uuid.eq(job_uuid)))
            .set(schema::job::last_heartbeat.eq(None::<DateTime>))
            .execute(&mut conn)
            .expect("Failed to clear last_heartbeat");
    }

    // Simulate server recovery: spawn heartbeat timeout for the claimed job
    let connection = Arc::new(Mutex::new(server.db_conn()));
    let heartbeat_tasks = HeartbeatTasks::new();
    let log = slog::Logger::root(slog::Discard, slog::o!());
    let timeout = std::time::Duration::from_secs(5);
    let grace_period = std::time::Duration::from_secs(60);

    // Pause time before spawning so the sleep timer is registered in virtual time
    tokio::time::pause();

    spawn_heartbeat_timeout(
        log,
        timeout,
        connection,
        job_id,
        &heartbeat_tasks,
        grace_period,
        bencher_json::Clock::System,
    );

    // Let the spawned task start and register its sleep(5s) timer
    tokio::task::yield_now().await;
    // Advance past the 5s timeout
    tokio::time::advance(std::time::Duration::from_secs(6)).await;
    // Let the spawned task complete after the timer fires
    tokio::task::yield_now().await;
    tokio::time::resume();

    // Verify the job is now Failed (heartbeat timeout, no WS interaction)
    let mut conn = server.db_conn();
    let status: JobStatus = schema::job::table
        .filter(schema::job::uuid.eq(job_uuid))
        .select(schema::job::status)
        .first(&mut conn)
        .expect("Failed to get job status");
    assert_eq!(
        status,
        JobStatus::Failed,
        "Claimed job should be Failed after heartbeat timeout without WS"
    );
}
