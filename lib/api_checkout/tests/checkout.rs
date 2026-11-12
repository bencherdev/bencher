#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for the checkout endpoint.
//!
//! `TestServer::new_with_cloud_routes` registers checkout without a biller, so a body
//! that checkout can sell reaches the missing biller (423), and one it cannot sell is
//! refused first (400).
#![cfg(feature = "plus")]

use bencher_api_tests::TestServer;
use bencher_json::{Entitlements, OrganizationUuid};
use http::StatusCode;

/// Post a checkout at `level`, with the entitlements and Self-Hosted organization of a
/// `license` if given.
#[expect(clippy::expect_used, reason = "test helper")]
async fn post_checkout(
    level: &str,
    license: Option<(Entitlements, OrganizationUuid)>,
) -> StatusCode {
    let server = TestServer::new_with_cloud_routes().await;
    let user = server.signup("Test User", "checkout@example.com").await;
    let org = server.create_org(&user, "Checkout Org").await;

    let (entitlements, self_hosted) = license.unzip();
    server
        .client
        .post(server.api_url("/v0/checkout"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&serde_json::json!({
            "organization": org.slug,
            "level": level,
            "entitlements": entitlements,
            "self_hosted": self_hosted,
        }))
        .send()
        .await
        .expect("Request failed")
        .status()
}

// Fails if checkout tries to sell an Enterprise plan without entitlements.
#[tokio::test]
async fn checkout_refuses_metered_enterprise() {
    assert_eq!(
        post_checkout("enterprise", None).await,
        StatusCode::BAD_REQUEST
    );
}

// Fails if checkout tries to sell the Free plan.
#[tokio::test]
async fn checkout_refuses_free() {
    assert_eq!(post_checkout("free", None).await, StatusCode::BAD_REQUEST);
}

// Fails if checkout accepts the Team plan level.
#[tokio::test]
async fn checkout_refuses_team() {
    assert_eq!(post_checkout("team", None).await, StatusCode::BAD_REQUEST);
}

// Fails if checkout refuses a plan it sells before it reaches the biller.
#[tokio::test]
async fn checkout_takes_pro_and_licensed_enterprise() {
    assert_eq!(post_checkout("pro", None).await, StatusCode::LOCKED);
    assert_eq!(
        post_checkout(
            "enterprise",
            Some((Entitlements::MIN, OrganizationUuid::new()))
        )
        .await,
        StatusCode::LOCKED
    );
}
