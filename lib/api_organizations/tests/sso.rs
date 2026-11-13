#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for organization single sign-on (SSO) domains.

use bencher_api_tests::{
    TestOrg, TestServer, TestUser,
    license::{DAY, TEST_LICENSE_SERIES, after, sign_license},
};
use http::StatusCode;

async fn add_sso(
    server: &TestServer,
    admin: &TestUser,
    org: &TestOrg,
    domain: &str,
) -> (StatusCode, String) {
    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/organizations/{org_slug}/sso")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .json(&serde_json::json!({ "domain": domain }))
        .send()
        .await
        .expect("Request failed");
    let status = resp.status();
    (status, resp.text().await.expect("Failed to read response"))
}

// POST /v0/organizations/{organization}/sso - on a Self-Hosted server, SSO takes a valid license
// on the organization, and the refusal names Enterprise.
#[tokio::test]
async fn sso_on_self_hosted_needs_a_valid_license() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin User", "ssoadmin@example.com").await;
    let org = server.create_org(&admin, "Sso License Org").await;

    let (status, body) = add_sso(&server, &admin, &org, "none.example.com").await;
    assert_eq!(status, StatusCode::PAYMENT_REQUIRED, "no license: {body}");
    assert!(body.contains("Enterprise"), "{body}");

    let now = server.now();
    server.store_license(
        &org,
        &sign_license(
            org.uuid,
            TEST_LICENSE_SERIES,
            after(now, -400 * DAY),
            after(now, -31 * DAY),
        ),
    );
    let (status, body) = add_sso(&server, &admin, &org, "expired.example.com").await;
    assert_eq!(
        status,
        StatusCode::PAYMENT_REQUIRED,
        "an expired license: {body}"
    );
    assert!(body.contains("Enterprise"), "{body}");

    server.license_org(&admin, &org).await;
    let (status, body) = add_sso(&server, &admin, &org, "valid.example.com").await;
    assert_eq!(status, StatusCode::ACCEPTED, "a valid license: {body}");
}
