#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for organization single sign-on (SSO) domains.

use bencher_api_tests::{
    LogCapture, TestOrg, TestServer, TestServerOptions, TestUser,
    helpers::{insert_licensed_plan, insert_metered_plan, insert_sso, user_organizations},
    license::{DAY, TEST_LICENSE_SERIES, after, sign_foreign_license, sign_license, stripe_cloud},
    stripe_stub::{self as stub, StripeStub, StubObjects, deal_item, pro_item},
};
use bencher_json::JsonConfirm;
use bencher_schema::model::organization::sso::ENTERPRISE_READ_TIMEOUT;
use http::StatusCode;
use serde_json::Value;

/// Bencher Cloud, whose every Stripe request goes to `stub`, logging to `capture`.
async fn cloud(stub: &StripeStub, capture: &LogCapture) -> TestServer {
    TestServer::new_with_options(TestServerOptions {
        cloud_routes: true,
        cloud: Some(stripe_cloud(stub)),
        log: Some(capture.logger()),
        ..TestServerOptions::default()
    })
    .await
}

/// Stripe holds `subscription` for the organization, with `plan_item` and in `status`.
fn bill(stub: &StripeStub, org: &TestOrg, subscription: &str, status: &str, plan_item: Value) {
    stub.set_subscription(stub::cloud_plan(
        subscription,
        status,
        plan_item,
        &org.uuid.to_string(),
    ));
}

/// Whether the user is a member of the organization.
fn member(server: &TestServer, user: &TestUser, org: &TestOrg) -> bool {
    user_organizations(server, user.uuid).contains(&org.uuid)
}

/// Confirm a login link for the user, which joins the organizations of their email's SSO domain.
async fn log_in(server: &TestServer, user: &TestUser) {
    let token = server
        .token_key()
        .new_auth(user.email.clone(), u32::MAX)
        .expect("Failed to create an auth token");
    let resp = server
        .client
        .post(server.api_url("/v0/auth/confirm"))
        .json(&JsonConfirm { token })
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK, "Failed to log in");
}

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

// POST /v0/organizations/{organization}/sso - on Bencher Cloud, SSO takes an active Enterprise
// plan, and the refusal names Enterprise.
#[tokio::test]
async fn sso_on_cloud_needs_an_active_enterprise_plan() {
    let stub = StripeStub::new(StubObjects::default()).await;
    let server = cloud(&stub, &LogCapture::default()).await;
    let admin = server
        .signup("Admin User", "cloudssoadmin@example.com")
        .await;
    let org = server.create_org(&admin, "Cloud Sso Org").await;

    let (status, body) = add_sso(&server, &admin, &org, "none.example.com").await;
    assert_eq!(status, StatusCode::PAYMENT_REQUIRED, "no plan: {body}");
    assert!(body.contains("Enterprise"), "{body}");

    insert_metered_plan(&server, &org, "sub_sso");
    bill(&stub, &org, "sub_sso", "active", pro_item());
    let (status, body) = add_sso(&server, &admin, &org, "pro.example.com").await;
    assert_eq!(status, StatusCode::PAYMENT_REQUIRED, "a Pro plan: {body}");
    assert!(body.contains("Enterprise"), "{body}");

    bill(&stub, &org, "sub_sso", "past_due", deal_item());
    let (status, body) = add_sso(&server, &admin, &org, "past-due.example.com").await;
    assert_eq!(
        status,
        StatusCode::PAYMENT_REQUIRED,
        "an Enterprise plan past due: {body}"
    );

    bill(&stub, &org, "sub_sso", "active", deal_item());
    let (status, body) = add_sso(&server, &admin, &org, "enterprise.example.com").await;
    assert_eq!(status, StatusCode::ACCEPTED, "an Enterprise plan: {body}");
}

// Fails if, on Bencher Cloud, a sign-up at an SSO domain joins an organization that is not on an
// active Enterprise plan, such as one that buys a Self-Hosted deal, or does not join one that is;
// if a sign-up that no SSO domain matches reads a plan from Stripe; or if a member leaves, or a new
// user still joins, once the organization is no longer Enterprise.
#[tokio::test]
async fn sso_joins_only_an_enterprise_organization_on_cloud() {
    let stub = StripeStub::new(StubObjects::default()).await;
    let server = cloud(&stub, &LogCapture::default()).await;
    let admin = server
        .signup("Admin User", "cloudjoinadmin@example.com")
        .await;
    let enterprise = server.create_org(&admin, "Enterprise Join Org").await;
    insert_metered_plan(&server, &enterprise, "sub_enterprise");
    bill(&stub, &enterprise, "sub_enterprise", "active", deal_item());
    let (status, body) = add_sso(&server, &admin, &enterprise, "enterprise.example.com").await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    let pro = server.create_org(&admin, "Pro Join Org").await;
    insert_metered_plan(&server, &pro, "sub_pro");
    bill(&stub, &pro, "sub_pro", "active", pro_item());
    insert_sso(&server, &pro, "pro.example.com");
    let free = server.create_org(&admin, "Free Join Org").await;
    insert_sso(&server, &free, "free.example.com");
    let deal = server.create_org(&admin, "Deal Join Org").await;
    let now = server.now();
    insert_licensed_plan(
        &server,
        &deal,
        "sub_deal",
        &sign_license(deal.uuid, 500, now, after(now, 365 * DAY)),
    );
    stub.set_subscription(stub::self_hosted_deal(
        "sub_deal",
        "active",
        &deal.uuid.to_string(),
    ));
    insert_sso(&server, &deal, "deal.example.com");

    let reads = stub.requests().len();
    server
        .signup("Elsewhere User", "user@elsewhere.example.com")
        .await;
    assert_eq!(stub.requests().len(), reads, "no SSO domain, no plan read");

    let joined = server
        .signup("Enterprise User", "user@enterprise.example.com")
        .await;
    assert!(member(&server, &joined, &enterprise));
    assert!(stub.requests().len() > reads, "the join read the plan");
    let pro_user = server.signup("Pro User", "user@pro.example.com").await;
    assert!(!member(&server, &pro_user, &pro));
    let free_user = server.signup("Free User", "user@free.example.com").await;
    assert!(!member(&server, &free_user, &free));
    let deal_user = server.signup("Deal User", "user@deal.example.com").await;
    assert!(!member(&server, &deal_user, &deal), "a Self-Hosted deal");

    bill(&stub, &enterprise, "sub_enterprise", "active", pro_item());
    log_in(&server, &joined).await;
    assert!(member(&server, &joined, &enterprise), "a member stays");
    let late = server
        .signup("Late User", "late@enterprise.example.com")
        .await;
    assert!(!member(&server, &late, &enterprise), "no one new joins");
}

// Fails if a sign-in fails, or joins the organization, when Bencher Cloud cannot read the plan of
// the organization at the user's SSO domain, or if it does not log why the user did not join.
#[tokio::test]
async fn sso_join_skips_when_the_plan_read_fails() {
    let stub = StripeStub::new(StubObjects::default()).await;
    let capture = LogCapture::default();
    let server = cloud(&stub, &capture).await;
    let admin = server
        .signup("Admin User", "brokenjoinadmin@example.com")
        .await;
    let org = server.create_org(&admin, "Broken Join Org").await;
    // Stripe has no such subscription.
    insert_metered_plan(&server, &org, "sub_missing");
    insert_sso(&server, &org, "broken.example.com");

    let user = server
        .signup("Broken User", "user@broken.example.com")
        .await;

    assert!(!member(&server, &user, &org));
    let organization = org.uuid.to_string();
    assert!(
        capture
            .lines()
            .iter()
            .any(|line| line.contains(&organization) && line.contains("did not join")),
        "{:?}",
        capture.lines()
    );
}

// Fails if a sign-in waits on a Stripe that stalls for longer than the join's own limit, joins the
// organization, or does not log why the user did not join. Time is paused, so the limit and the
// stall pass in virtual time, in order.
#[tokio::test]
async fn sso_join_gives_up_on_a_stalled_plan_read() {
    let stub = StripeStub::new(StubObjects::default()).await;
    let capture = LogCapture::default();
    let server = cloud(&stub, &capture).await;
    let admin = server
        .signup("Admin User", "stalledjoinadmin@example.com")
        .await;
    let org = server.create_org(&admin, "Stalled Join Org").await;
    insert_metered_plan(&server, &org, "sub_stalled");
    bill(&stub, &org, "sub_stalled", "active", deal_item());
    insert_sso(&server, &org, "stalled.example.com");
    stub.delay(ENTERPRISE_READ_TIMEOUT * 4);

    tokio::time::pause();
    let started = tokio::time::Instant::now();
    let user = server
        .signup("Stalled User", "user@stalled.example.com")
        .await;
    let waited = started.elapsed();
    tokio::time::resume();

    assert!(
        waited >= ENTERPRISE_READ_TIMEOUT,
        "the sign-in waited {waited:?}"
    );
    assert!(!member(&server, &user, &org));
    let organization = org.uuid.to_string();
    assert!(
        capture
            .lines()
            .iter()
            .any(|line| line.contains(&organization) && line.contains("did not join")),
        "{:?}",
        capture.lines()
    );
}

// Fails if, on Bencher Self-Hosted, a sign-up at an SSO domain joins an organization whose
// license is expired, signed by another key, or missing, or does not join one with a valid
// license.
#[tokio::test]
async fn sso_joins_only_a_licensed_organization_on_self_hosted() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin User", "shjoinadmin@example.com").await;
    let now = server.now();
    let licensed = server.create_org(&admin, "Licensed Join Org").await;
    server.license_org(&admin, &licensed).await;
    let (status, body) = add_sso(&server, &admin, &licensed, "licensed.example.com").await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    let expired = server.create_org(&admin, "Expired Join Org").await;
    server.store_license(
        &expired,
        &sign_license(
            expired.uuid,
            TEST_LICENSE_SERIES,
            after(now, -400 * DAY),
            after(now, -31 * DAY),
        ),
    );
    insert_sso(&server, &expired, "expired.example.com");
    let foreign = server.create_org(&admin, "Foreign Join Org").await;
    server.store_license(
        &foreign,
        &sign_foreign_license(
            foreign.uuid,
            TEST_LICENSE_SERIES,
            now,
            after(now, 365 * DAY),
        ),
    );
    insert_sso(&server, &foreign, "foreign.example.com");
    let unlicensed = server.create_org(&admin, "Unlicensed Join Org").await;
    insert_sso(&server, &unlicensed, "unlicensed.example.com");

    let user = server
        .signup("Licensed User", "user@licensed.example.com")
        .await;
    assert!(member(&server, &user, &licensed));
    let user = server
        .signup("Expired User", "user@expired.example.com")
        .await;
    assert!(!member(&server, &user, &expired));
    let user = server
        .signup("Foreign User", "user@foreign.example.com")
        .await;
    assert!(!member(&server, &user, &foreign));
    let user = server
        .signup("Unlicensed User", "user@unlicensed.example.com")
        .await;
    assert!(!member(&server, &user, &unlicensed));
}
