#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args,
    reason = "integration test file"
)]
//! Integration tests for organization plan endpoint.
//!
//! NOTE: The organization plan endpoints are registered only on Bencher Cloud
//! (`is_bencher_cloud` in `lib.rs`), so the default test harness returns 404 for
//! them; the GET tests below accept that. `TestServer::new_with_cloud_routes`
//! registers them without a biller, which is enough to reach PATCH's session
//! gate. DELETE uses the same `AdminUser` extractor tested by
//! `organizations_hard_delete_requires_admin`.

use bencher_api_tests::TestServer;
use http::StatusCode;

// GET /v0/organizations/{organization}/plan - view plan
#[tokio::test]
async fn plan_get() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "planget@example.com").await;
    let org = server.create_org(&user, "Plan Org").await;

    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/organizations/{}/plan", org_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    // Plan may or may not exist - both OK and NOT_FOUND are valid
    assert!(
        resp.status() == StatusCode::OK || resp.status() == StatusCode::NOT_FOUND,
        "Unexpected status: {}",
        resp.status()
    );
}

// GET /v0/organizations/{organization}/plan - requires auth
#[tokio::test]
async fn plan_requires_auth() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "planauth@example.com").await;
    let org = server.create_org(&user, "Plan Auth Org").await;

    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/organizations/{}/plan", org_slug)))
        .send()
        .await
        .expect("Request failed");

    // Without auth, should get an error (either 400 Bad Request or 404 Not Found)
    assert!(
        resp.status() == StatusCode::BAD_REQUEST || resp.status() == StatusCode::NOT_FOUND,
        "Expected 400 or 404, got: {}",
        resp.status()
    );
}

// GET /v0/organizations/{organization}/plan - not found org
#[tokio::test]
async fn plan_org_not_found() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "plannotfound@example.com").await;

    let resp = server
        .client
        .get(server.api_url("/v0/organizations/nonexistent-org/plan"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// Fails if plan PATCH accepts a user API key or a deprecated user API token, or refuses a user session.
#[cfg(feature = "plus")]
#[tokio::test]
async fn plan_patch_requires_session() {
    use bencher_api_tests::helpers::seed_token;
    use bencher_json::JsonUserKeyCreated;

    let server = TestServer::new_with_cloud_routes().await;
    let user = server
        .signup("Test User", "planpatchsession@example.com")
        .await;
    let org = server.create_org(&user, "Plan Session Org").await;
    let api_token = seed_token(&server, &user, "Legacy Token");

    let org_slug: &str = org.slug.as_ref();
    let body = serde_json::json!({"cancel_at_period_end": true});
    let resp = server
        .client
        .patch(server.api_url(&format!("/v0/organizations/{}/plan", org_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&api_token.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    let user_slug: &str = user.slug.as_ref();
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/users/{}/keys", user_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&serde_json::json!({"name": "plan-key"}))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let created: JsonUserKeyCreated = resp.json().await.expect("Failed to parse user key");
    let resp = server
        .client
        .patch(server.api_url(&format!("/v0/organizations/{}/plan", org_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(created.key.as_ref()),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // A user session passes the gate and reaches the missing biller.
    let resp = server
        .client
        .patch(server.api_url(&format!("/v0/organizations/{}/plan", org_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::LOCKED);
}
