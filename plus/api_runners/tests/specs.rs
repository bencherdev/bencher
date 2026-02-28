#![expect(unused_crate_dependencies, clippy::tests_outside_test_module)]
//! Integration tests for runner-spec association endpoints and spec-based claim matching.

mod common;

use bencher_api_tests::TestServer;
use bencher_json::{JsonClaimedJob, JsonSpec, JsonSpecs};
use common::{
    associate_runner_spec, create_runner, create_test_report, get_project_id, get_runner_id,
    insert_test_job, insert_test_spec, insert_test_spec_full,
};
use http::StatusCode;

// POST /v0/runners/{runner}/specs - add spec to runner
#[tokio::test]
async fn runner_specs_add() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "rspecadd@example.com").await;

    let runner = create_runner(&server, &admin.token, "Spec Add Runner").await;
    let (spec_uuid, _spec_id) = insert_test_spec(&server);

    let body = serde_json::json!({"spec": spec_uuid.to_string()});
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/specs", runner.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
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
async fn runner_specs_list() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "rspeclist@example.com").await;

    let runner = create_runner(&server, &admin.token, "Spec List Runner").await;
    let (spec_uuid, _spec_id) = insert_test_spec(&server);

    // Add spec to runner via API
    let body = serde_json::json!({"spec": spec_uuid.to_string()});
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/specs", runner.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    // List specs
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/runners/{}/specs", runner.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
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
async fn runner_specs_remove() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "rspecremove@example.com").await;

    let runner = create_runner(&server, &admin.token, "Spec Remove Runner").await;
    let (spec_uuid, _spec_id) = insert_test_spec(&server);

    // Add spec to runner
    let body = serde_json::json!({"spec": spec_uuid.to_string()});
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/specs", runner.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Remove spec from runner
    let resp = server
        .client
        .delete(server.api_url(&format!("/v0/runners/{}/specs/{}", runner.uuid, spec_uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Verify it's gone
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/runners/{}/specs", runner.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .send()
        .await
        .expect("Request failed");

    let specs: JsonSpecs = resp.json().await.expect("Failed to parse response");
    let found = specs.0.iter().any(|s| s.uuid == spec_uuid);
    assert!(!found, "Spec should not appear after removal");
}

// DELETE /v0/runners/{runner}/specs/{spec} - nonexistent association returns 404
#[tokio::test]
async fn runner_specs_remove_nonexistent() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "rspecrmne@example.com").await;

    let runner = create_runner(&server, &admin.token, "Spec Remove NE Runner").await;
    let (spec_uuid, _spec_id) = insert_test_spec(&server);

    // Try to remove a spec that was never associated
    let resp = server
        .client
        .delete(server.api_url(&format!("/v0/runners/{}/specs/{}", runner.uuid, spec_uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// POST /v0/runners/{runner}/specs - non-admin gets 403
#[tokio::test]
async fn runner_specs_add_forbidden_for_non_admin() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "rspecforbid@example.com").await;
    let user = server.signup("User", "rspecuser@example.com").await;

    let runner = create_runner(&server, &admin.token, "Forbidden Spec Runner").await;
    let (spec_uuid, _spec_id) = insert_test_spec(&server);

    let body = serde_json::json!({"spec": spec_uuid.to_string()});
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/specs", runner.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// POST /v0/runners/{runner}/specs - duplicate association returns conflict
#[tokio::test]
async fn runner_specs_add_duplicate() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "rspecdup@example.com").await;

    let runner = create_runner(&server, &admin.token, "Spec Dup Runner").await;
    let (spec_uuid, _spec_id) = insert_test_spec(&server);

    // First association should succeed
    let body = serde_json::json!({"spec": spec_uuid.to_string()});
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/specs", runner.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Second association of the same spec should fail (UNIQUE constraint)
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/specs", runner.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(
        resp.status(),
        StatusCode::CONFLICT,
        "Duplicate spec association should be rejected"
    );
}

// GET /v0/runners/{runner}/specs - returns all associated specs
#[tokio::test]
async fn runner_specs_list_multiple() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "rspecmulti@example.com").await;

    let runner = create_runner(&server, &admin.token, "Spec Multi Runner").await;

    // Create 3 different specs
    let (spec1_uuid, _) =
        insert_test_spec_full(&server, "x86_64", 1, 0x8000_0000, 5_368_709_120, false);
    let (spec2_uuid, _) = insert_test_spec_full(
        &server,
        "aarch64",
        2,
        0x0001_0000_0000,
        10_737_418_240,
        false,
    );
    let (spec3_uuid, _) =
        insert_test_spec_full(&server, "x86_64", 4, 0x0002_0000_0000, 21_474_836_480, true);

    // Associate all 3 specs with the runner
    for spec_uuid in [spec1_uuid, spec2_uuid, spec3_uuid] {
        let body = serde_json::json!({"spec": spec_uuid.to_string()});
        let resp = server
            .client
            .post(server.api_url(&format!("/v0/runners/{}/specs", runner.uuid)))
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(&admin.token),
            )
            .json(&body)
            .send()
            .await
            .expect("Request failed");
        assert_eq!(resp.status(), StatusCode::CREATED);
    }

    // List specs
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/runners/{}/specs", runner.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let specs: JsonSpecs = resp.json().await.expect("Failed to parse response");
    assert_eq!(specs.0.len(), 3, "All 3 specs should be returned");

    // Verify all spec UUIDs are present
    let uuids: Vec<_> = specs.0.iter().map(|s| s.uuid).collect();
    assert!(uuids.contains(&spec1_uuid));
    assert!(uuids.contains(&spec2_uuid));
    assert!(uuids.contains(&spec3_uuid));
}

// GET /v0/runners/{runner}/specs - empty when no specs associated
#[tokio::test]
async fn runner_specs_list_empty() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "rspecempty@example.com").await;

    let runner = create_runner(&server, &admin.token, "Spec Empty Runner").await;

    let resp = server
        .client
        .get(server.api_url(&format!("/v0/runners/{}/specs", runner.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let specs: JsonSpecs = resp.json().await.expect("Failed to parse response");
    assert!(
        specs.0.is_empty(),
        "Runner with no specs should return empty list"
    );
}

// Claim job - runner with no specs at all cannot claim a pending job
#[tokio::test]
async fn claim_job_no_specs() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "nospec@example.com").await;
    let org = server.create_org(&admin, "No Spec Org").await;
    let project = server.create_project(&admin, &org, "No Spec Project").await;

    // Create runner but do NOT associate any specs
    let runner = create_runner(&server, &admin.token, "No Spec Runner").await;
    let runner_token: &str = runner.token.as_ref();

    // Create a pending job
    let (_, spec_id) = insert_test_spec(&server);
    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let _job_uuid = insert_test_job(&server, report_id, spec_id);

    // Runner tries to claim - should get None (no specs associated)
    let claim_body = serde_json::json!({"poll_timeout": 1});
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(runner_token),
        )
        .json(&claim_body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let claimed: Option<JsonClaimedJob> = resp.json().await.expect("Failed to parse response");
    assert!(
        claimed.is_none(),
        "Runner with no specs should not claim any job"
    );
}

// Claim job - runner without matching spec cannot claim a pending job
#[tokio::test]
async fn claim_job_spec_mismatch() {
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
    let (_spec_x86_uuid, spec_x86_id) = insert_test_spec_full(
        &server,
        "x86_64",
        2,
        0x0001_0000_0000,
        10_737_418_240,
        false,
    );
    let (_spec_arm_uuid, spec_arm_id) = insert_test_spec_full(
        &server,
        "aarch64",
        4,
        0x0002_0000_0000,
        21_474_836_480,
        true,
    );

    // Associate runner with spec x86 only
    associate_runner_spec(&server, runner_id, spec_x86_id);

    // Create a job referencing spec arm
    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let _job_uuid = insert_test_job(&server, report_id, spec_arm_id);

    // Runner tries to claim - should get None (no matching job for its specs)
    let claim_body = serde_json::json!({"poll_timeout": 1});
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(runner_token),
        )
        .json(&claim_body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let claimed: Option<JsonClaimedJob> = resp.json().await.expect("Failed to parse response");
    assert!(
        claimed.is_none(),
        "Runner should not claim job with mismatched spec"
    );

    // Now associate runner with spec arm
    associate_runner_spec(&server, runner_id, spec_arm_id);

    // Runner tries to claim again - should get the job
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(runner_token),
        )
        .json(&claim_body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let claimed: Option<JsonClaimedJob> = resp.json().await.expect("Failed to parse response");
    assert!(
        claimed.is_some(),
        "Runner should claim job after spec association"
    );
}

// =============================================================================
// Runner Specs Get Endpoint Tests
// =============================================================================

// GET /v0/runners/{runner}/specs - list returns specs after adding them via API
#[tokio::test]
async fn runner_specs_get_after_add() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "rspecget@example.com").await;

    let runner = create_runner(&server, &admin.token, "Spec Get Runner").await;
    let (spec_uuid, _spec_id) = insert_test_spec(&server);

    // Add spec via API
    let body = serde_json::json!({"spec": spec_uuid.to_string()});
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/specs", runner.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    // List specs via GET
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/runners/{}/specs", runner.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let specs: JsonSpecs = resp.json().await.expect("Failed to parse response");
    assert_eq!(specs.0.len(), 1);
    let first_spec = specs.0.first().expect("Expected at least one spec");
    assert_eq!(first_spec.uuid, spec_uuid);
}

// GET /v0/runners/{runner}/specs - empty list after spec removal
#[tokio::test]
async fn runner_specs_get_after_removal() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "rspecgetrem@example.com").await;

    let runner = create_runner(&server, &admin.token, "Spec GetRem Runner").await;
    let (spec_uuid, _spec_id) = insert_test_spec(&server);

    // Add spec
    let body = serde_json::json!({"spec": spec_uuid.to_string()});
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/specs", runner.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Remove spec
    let resp = server
        .client
        .delete(server.api_url(&format!("/v0/runners/{}/specs/{}", runner.uuid, spec_uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // List should now be empty
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/runners/{}/specs", runner.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let specs: JsonSpecs = resp.json().await.expect("Failed to parse response");
    assert!(specs.0.is_empty(), "Specs should be empty after removal");
}

// GET /v0/runners/{runner}/specs - multiple specs returns all of them
#[tokio::test]
async fn runner_specs_get_multiple() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "rspecgetmulti@example.com").await;

    let runner = create_runner(&server, &admin.token, "Spec GetMulti Runner").await;

    let (spec1_uuid, _) =
        insert_test_spec_full(&server, "x86_64", 1, 0x8000_0000, 5_368_709_120, false);
    let (spec2_uuid, _) = insert_test_spec_full(
        &server,
        "aarch64",
        2,
        0x0001_0000_0000,
        10_737_418_240,
        true,
    );

    // Add both specs
    for spec_uuid in [spec1_uuid, spec2_uuid] {
        let body = serde_json::json!({"spec": spec_uuid.to_string()});
        let resp = server
            .client
            .post(server.api_url(&format!("/v0/runners/{}/specs", runner.uuid)))
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(&admin.token),
            )
            .json(&body)
            .send()
            .await
            .expect("Request failed");
        assert_eq!(resp.status(), StatusCode::CREATED);
    }

    // List should have 2
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/runners/{}/specs", runner.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let specs: JsonSpecs = resp.json().await.expect("Failed to parse response");
    assert_eq!(specs.0.len(), 2);
    let uuids: Vec<_> = specs.0.iter().map(|s| s.uuid).collect();
    assert!(uuids.contains(&spec1_uuid));
    assert!(uuids.contains(&spec2_uuid));
}

// GET /v0/runners/{runner}/specs - non-admin gets 403
#[tokio::test]
async fn runner_specs_get_forbidden_for_non_admin() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "rspecgetforbid@example.com").await;
    let user = server.signup("User", "rspecgetuser@example.com").await;

    let runner = create_runner(&server, &admin.token, "Spec GetForbid Runner").await;

    let resp = server
        .client
        .get(server.api_url(&format!("/v0/runners/{}/specs", runner.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}
