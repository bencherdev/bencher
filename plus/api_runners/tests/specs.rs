#![expect(unused_crate_dependencies, clippy::tests_outside_test_module)]
//! Integration tests for runner-spec association endpoints and spec-based claim matching.

mod common;

use bencher_api_tests::TestServer;
use bencher_json::{
    JsonJob,
    runner::{JsonSpec, JsonSpecs},
};
use common::{
    associate_runner_spec, create_runner, create_test_report, get_project_id, get_runner_id,
    insert_test_job, insert_test_spec, insert_test_spec_full,
};
use http::StatusCode;

// POST /v0/runners/{runner}/specs - add spec to runner
#[tokio::test]
async fn test_runner_specs_add() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "rspecadd@example.com").await;

    let runner = create_runner(&server, &admin.token, "Spec Add Runner").await;
    let (spec_uuid, _spec_id) = insert_test_spec(&server);

    let body = serde_json::json!({"spec": spec_uuid.to_string()});
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/specs", runner.uuid)))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
    let spec: JsonSpec = resp.json().await.expect("Failed to parse response");
    assert_eq!(spec.uuid, spec_uuid);
}

// GET /v0/runners/{runner}/specs - list runner specs
#[tokio::test]
async fn test_runner_specs_list() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "rspeclist@example.com").await;

    let runner = create_runner(&server, &admin.token, "Spec List Runner").await;
    let (spec_uuid, _spec_id) = insert_test_spec(&server);

    // Add spec to runner via API
    let body = serde_json::json!({"spec": spec_uuid.to_string()});
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/specs", runner.uuid)))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    // List specs
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/runners/{}/specs", runner.uuid)))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let specs: JsonSpecs = resp.json().await.expect("Failed to parse response");
    let found = specs.0.iter().any(|s| s.uuid == spec_uuid);
    assert!(found, "Spec should appear in runner's spec list");
}

// DELETE /v0/runners/{runner}/specs/{spec} - remove spec from runner
#[tokio::test]
async fn test_runner_specs_remove() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "rspecremove@example.com").await;

    let runner = create_runner(&server, &admin.token, "Spec Remove Runner").await;
    let (spec_uuid, _spec_id) = insert_test_spec(&server);

    // Add spec to runner
    let body = serde_json::json!({"spec": spec_uuid.to_string()});
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/specs", runner.uuid)))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Remove spec from runner
    let resp = server
        .client
        .delete(server.api_url(&format!("/v0/runners/{}/specs/{}", runner.uuid, spec_uuid)))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Verify it's gone
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/runners/{}/specs", runner.uuid)))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");

    let specs: JsonSpecs = resp.json().await.expect("Failed to parse response");
    let found = specs.0.iter().any(|s| s.uuid == spec_uuid);
    assert!(!found, "Spec should not appear after removal");
}

// DELETE /v0/runners/{runner}/specs/{spec} - nonexistent association returns 404
#[tokio::test]
async fn test_runner_specs_remove_nonexistent() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "rspecrmne@example.com").await;

    let runner = create_runner(&server, &admin.token, "Spec Remove NE Runner").await;
    let (spec_uuid, _spec_id) = insert_test_spec(&server);

    // Try to remove a spec that was never associated
    let resp = server
        .client
        .delete(server.api_url(&format!("/v0/runners/{}/specs/{}", runner.uuid, spec_uuid)))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// POST /v0/runners/{runner}/specs - non-admin gets 403
#[tokio::test]
async fn test_runner_specs_add_forbidden_for_non_admin() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "rspecforbid@example.com").await;
    let user = server.signup("User", "rspecuser@example.com").await;

    let runner = create_runner(&server, &admin.token, "Forbidden Spec Runner").await;
    let (spec_uuid, _spec_id) = insert_test_spec(&server);

    let body = serde_json::json!({"spec": spec_uuid.to_string()});
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/specs", runner.uuid)))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// Claim job - runner without matching spec cannot claim a pending job
#[tokio::test]
async fn test_claim_job_spec_mismatch() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "specmismatch@example.com").await;
    let org = server.create_org(&admin, "Spec Mismatch Org").await;
    let project = server
        .create_project(&admin, &org, "Spec Mismatch Project")
        .await;

    // Create runner
    let runner = create_runner(&server, &admin.token, "Mismatch Runner").await;
    let runner_token: &str = runner.token.as_ref();
    let runner_id = get_runner_id(&server, runner.uuid);

    // Create two specs with different configurations
    let (_spec_a_uuid, spec_a_id) =
        insert_test_spec_full(&server, 2, 4_294_967_296, 10_737_418_240, false);
    let (_spec_b_uuid, spec_b_id) =
        insert_test_spec_full(&server, 4, 8_589_934_592, 21_474_836_480, true);

    // Associate runner with spec A only
    associate_runner_spec(&server, runner_id, spec_a_id);

    // Create a job referencing spec B
    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let _job_uuid = insert_test_job(&server, report_id, spec_b_id);

    // Runner tries to claim - should get None (no matching job for its specs)
    let claim_body = serde_json::json!({"poll_timeout": 1});
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
        .header("Authorization", format!("Bearer {runner_token}"))
        .json(&claim_body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let claimed: Option<JsonJob> = resp.json().await.expect("Failed to parse response");
    assert!(
        claimed.is_none(),
        "Runner should not claim job with mismatched spec"
    );

    // Now associate runner with spec B
    associate_runner_spec(&server, runner_id, spec_b_id);

    // Runner tries to claim again - should get the job
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
        .header("Authorization", format!("Bearer {runner_token}"))
        .json(&claim_body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let claimed: Option<JsonJob> = resp.json().await.expect("Failed to parse response");
    assert!(
        claimed.is_some(),
        "Runner should claim job after spec association"
    );
}
