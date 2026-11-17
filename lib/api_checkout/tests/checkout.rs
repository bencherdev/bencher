#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for the checkout endpoint.
//!
//! `TestServer::new_with_cloud_routes` registers checkout without a biller, so a body
//! that checkout takes reaches the missing biller (423).
#![cfg(feature = "plus")]

use bencher_api_tests::{TestServer, TestUser, helpers::grant_organization_role};
use bencher_rbac::organization::Role;
use http::StatusCode;

#[expect(clippy::expect_used, reason = "test helper")]
async fn post_checkout(
    server: &TestServer,
    user: &TestUser,
    body: &serde_json::Value,
) -> StatusCode {
    server
        .client
        .post(server.api_url("/v0/checkout"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(body)
        .send()
        .await
        .expect("Request failed")
        .status()
}

// Fails if checkout needs anything but the organization to start a Pro checkout, or
// refuses the body of an earlier console that still names its level.
#[tokio::test]
async fn checkout_takes_the_organization() {
    let server = TestServer::new_with_cloud_routes().await;
    let user = server.signup("Test User", "checkout@example.com").await;
    let org = server.create_org(&user, "Checkout Org").await;

    for body in [
        serde_json::json!({ "organization": org.slug }),
        serde_json::json!({ "organization": org.slug, "level": "pro" }),
    ] {
        assert_eq!(
            post_checkout(&server, &user, &body).await,
            StatusCode::LOCKED,
            "{body}"
        );
    }
}

// Fails if a checkout starts for anyone who cannot manage the organization: a user from
// outside it, or one of its Members.
#[tokio::test]
async fn checkout_requires_manage() {
    let server = TestServer::new_with_cloud_routes().await;
    let leader = server.signup("Leader", "checkoutleader@example.com").await;
    let org = server.create_org(&leader, "Checkout Manage Org").await;
    let member = server.signup("Member", "checkoutmember@example.com").await;
    grant_organization_role(&server, &member, &org, Role::Member);
    let outsider = server
        .signup("Outsider", "checkoutoutsider@example.com")
        .await;
    let body = serde_json::json!({ "organization": org.slug });

    assert_eq!(
        post_checkout(&server, &outsider, &body).await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        post_checkout(&server, &member, &body).await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        post_checkout(&server, &leader, &body).await,
        StatusCode::LOCKED
    );
}
