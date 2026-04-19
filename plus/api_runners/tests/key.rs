#![expect(unused_crate_dependencies, clippy::tests_outside_test_module)]
//! Integration tests for runner key rotation endpoint.

mod common;

use bencher_api_tests::TestServer;
use bencher_json::JsonRunnerKey;
use common::{
    assert_ws_closed, associate_runner_spec, claim_via_channel, create_runner, create_test_report,
    get_project_id, get_runner_id, insert_test_job, insert_test_spec, try_connect_channel_ws,
};
use futures_concurrency::future::Join as _;
use http::StatusCode;

// POST /v0/runners/{runner}/key - admin can rotate key
#[tokio::test]
async fn key_rotate_as_admin() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "keyadmin@example.com").await;

    // Create a runner
    let body = serde_json::json!({
        "name": "Key Rotate Runner"
    });
    let resp = server
        .client
        .post(server.api_url("/v0/runners"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    let original_key: JsonRunnerKey = resp.json().await.expect("Failed to parse response");

    // Rotate key
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/key", original_key.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
    let new_key: JsonRunnerKey = resp.json().await.expect("Failed to parse response");

    // UUID should be the same
    assert_eq!(new_key.uuid, original_key.uuid);
    // Key should be different
    let original_str: &str = original_key.key.as_ref();
    let new_str: &str = new_key.key.as_ref();
    assert_ne!(original_str, new_str);
    // New key should start with prefix
    assert!(new_str.starts_with("bencher_runner_"));
}

// POST /v0/runners/{runner}/key - non-admin cannot rotate key
#[tokio::test]
async fn key_rotate_forbidden_for_non_admin() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "keyadmin2@example.com").await;
    let user = server.signup("User", "keyuser@example.com").await;

    // Create a runner as admin
    let body = serde_json::json!({
        "name": "Key Test Runner"
    });
    let resp = server
        .client
        .post(server.api_url("/v0/runners"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    let runner_key: JsonRunnerKey = resp.json().await.expect("Failed to parse response");

    // Non-admin tries to rotate
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/key", runner_key.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// POST /v0/runners/{runner}/key - rotate by slug
#[tokio::test]
async fn key_rotate_by_slug() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "keyadmin3@example.com").await;

    // Create a runner with a slug
    let body = serde_json::json!({
        "name": "Key Slug Runner",
        "slug": "key-slug-runner"
    });
    let resp = server
        .client
        .post(server.api_url("/v0/runners"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    let original_key: JsonRunnerKey = resp.json().await.expect("Failed to parse response");

    // Rotate key by slug
    let resp = server
        .client
        .post(server.api_url("/v0/runners/key-slug-runner/key"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
    let new_key: JsonRunnerKey = resp.json().await.expect("Failed to parse response");
    assert_eq!(new_key.uuid, original_key.uuid);
}

// POST /v0/runners/{runner}/key - not found for invalid runner
#[tokio::test]
async fn key_rotate_not_found() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "keyadmin4@example.com").await;

    let resp = server
        .client
        .post(server.api_url("/v0/runners/nonexistent-runner/key"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// POST /v0/runners/{runner}/key - concurrent rotation yields two different keys
#[tokio::test]
async fn concurrent_key_rotation() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "keyadmin5@example.com").await;

    // Create a runner
    let body = serde_json::json!({ "name": "Concurrent Rotate Runner" });
    let resp = server
        .client
        .post(server.api_url("/v0/runners"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    let original: JsonRunnerKey = resp.json().await.expect("Failed to parse response");
    let original_str: String = original.key.as_ref().to_owned();

    // Two concurrent rotations
    let url = server.api_url(&format!("/v0/runners/{}/key", original.uuid));
    let bearer = bencher_json::bearer_header(&admin.token);
    let client = &server.client;

    let (resp1, resp2) = (
        async {
            client
                .post(&url)
                .header(bencher_json::AUTHORIZATION, &bearer)
                .send()
                .await
                .expect("Request 1 failed")
        },
        async {
            client
                .post(&url)
                .header(bencher_json::AUTHORIZATION, &bearer)
                .send()
                .await
                .expect("Request 2 failed")
        },
    )
        .join()
        .await;

    assert_eq!(resp1.status(), StatusCode::CREATED);
    assert_eq!(resp2.status(), StatusCode::CREATED);

    let key1: JsonRunnerKey = resp1.json().await.expect("Failed to parse response 1");
    let key2: JsonRunnerKey = resp2.json().await.expect("Failed to parse response 2");

    let k1: &str = key1.key.as_ref();
    let k2: &str = key2.key.as_ref();

    // Both keys should differ from the original
    assert_ne!(k1, original_str, "Key 1 should differ from original");
    assert_ne!(k2, original_str, "Key 2 should differ from original");

    // The two keys should differ from each other
    assert_ne!(k1, k2, "Concurrent rotations should produce different keys");

    // Verify only one of the two keys works for auth (the last writer wins)
    // We can't predict which one, but exactly one should authenticate.
    // We just verify both have the correct prefix and length.
    assert!(k1.starts_with("bencher_runner_"));
    assert!(k2.starts_with("bencher_runner_"));
    assert_eq!(k1.len(), api_runners::RUNNER_KEY_LENGTH);
    assert_eq!(k2.len(), api_runners::RUNNER_KEY_LENGTH);
}

// After key rotation, the old key should be rejected.
#[tokio::test]
async fn old_key_rejected_after_rotation() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "keyold@example.com").await;

    // Create a runner and save the original key
    let body = serde_json::json!({ "name": "Old Key Runner" });
    let resp = server
        .client
        .post(server.api_url("/v0/runners"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    let original: JsonRunnerKey = resp.json().await.expect("Failed to parse response");
    let original_key: String = original.key.as_ref().to_owned();

    // Rotate the key
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/key", original.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let new: JsonRunnerKey = resp.json().await.expect("Failed to parse response");
    let new_key: String = new.key.as_ref().to_owned();

    // Old key should be rejected on the WS channel endpoint
    let result = try_connect_channel_ws(&server, original.uuid, &original_key).await;
    match result {
        Err(_) => {}, // Connection refused — old key rejected
        Ok(mut ws) => {
            // Dropshot upgrades before auth, so connection may succeed but
            // immediately close
            assert_ws_closed(&mut ws).await;
        },
    }

    // New key should work on the WS channel endpoint
    let (_ws, _) = claim_via_channel(&server, original.uuid, &new_key, 1).await;
}

// Rotating a key on an archived runner should fail.
#[tokio::test]
async fn key_rotate_archived_runner() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "keyarchived@example.com").await;

    // Create a runner
    let body = serde_json::json!({ "name": "Archived Key Runner" });
    let resp = server
        .client
        .post(server.api_url("/v0/runners"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let runner: JsonRunnerKey = resp.json().await.expect("Failed to parse response");

    // Archive the runner
    let body = serde_json::json!({ "archived": true });
    let resp = server
        .client
        .patch(server.api_url(&format!("/v0/runners/{}", runner.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);

    // Try to rotate key on the archived runner — should fail
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/key", runner.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_ne!(
        resp.status(),
        StatusCode::CREATED,
        "Key rotation on archived runner should not succeed"
    );
}

// Rotating a key while the runner has in-flight jobs should succeed,
// and the old key should be invalidated.
#[tokio::test]
async fn key_rotate_with_inflight_jobs() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "keyinflight@example.com").await;
    let org = server.create_org(&admin, "Key Inflight Org").await;
    let project = server
        .create_project(&admin, &org, "Key Inflight Project")
        .await;

    // Create runner and set up infrastructure
    let runner = create_runner(&server, &admin.token, "Inflight Key Runner").await;
    let original_key: String = runner.key.as_ref().to_owned();
    let runner_id = get_runner_id(&server, runner.uuid);
    let (_, spec_id) = insert_test_spec(&server);
    associate_runner_spec(&server, runner_id, spec_id);

    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let _job_uuid = insert_test_job(&server, report_id, spec_id);

    // Claim the job via WS channel
    let (_ws, claimed) = claim_via_channel(&server, runner.uuid, &original_key, 5).await;
    assert!(claimed.is_some(), "Should claim a job");

    // Rotate key while job is in-flight (Claimed status)
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/key", runner.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "Key rotation should succeed even with in-flight jobs"
    );
    let new: JsonRunnerKey = resp.json().await.expect("Failed to parse response");
    let new_key: String = new.key.as_ref().to_owned();

    // Old key should be rejected on the WS channel endpoint
    let result = try_connect_channel_ws(&server, runner.uuid, &original_key).await;
    match result {
        Err(_) => {}, // Connection refused — old key rejected
        Ok(mut ws) => {
            assert_ws_closed(&mut ws).await;
        },
    }

    // New key should authenticate on the WS channel endpoint
    let (_ws, _) = claim_via_channel(&server, runner.uuid, &new_key, 1).await;
}
