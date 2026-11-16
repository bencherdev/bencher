#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for organization single sign-on (SSO) domains.

use std::time::Duration;

use bencher_api_tests::{
    LogCapture, TestOrg, TestServer, TestServerOptions, TestUser,
    helpers::{
        grant_organization_role, insert_licensed_plan, insert_metered_plan, insert_sso,
        user_organizations,
    },
    license::{DAY, TEST_LICENSE_SERIES, after, sign_foreign_license, sign_license, stripe_cloud},
    stripe_stub::{self as stub, StripeStub, StubObjects, deal_item, pro_item},
};
use bencher_rbac::organization::Role;
use bencher_schema::{
    model::{
        organization::sso::{ENTERPRISE_READ_TIMEOUT, QuerySso},
        user::QueryUser,
    },
    schema,
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use http::StatusCode;
use serde_json::Value;
use tokio::time::Instant;

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

/// Sign the user in by the code in a sign-in email, which the server logs to `capture`, and which
/// joins the organization of their email's SSO domain.
async fn log_in(server: &TestServer, capture: &LogCapture, user: &TestUser) {
    server.sign_in_by_email(user.email.as_ref(), capture).await;
}

/// A user at `domain` who signed up before it was the organization's SSO domain, so their sign-up
/// joined nothing, and then the domain.
async fn signed_up_before_sso(
    server: &TestServer,
    org: &TestOrg,
    name: &str,
    domain: &str,
) -> TestUser {
    let user = server.signup(name, &format!("user@{domain}")).await;
    insert_sso(server, org, domain);
    user
}

/// Bencher Cloud with an organization on an active Enterprise plan at the SSO domain `domain`, a
/// user at the domain who is not a member, and a Stripe that holds each answer back for a second,
/// with the count of requests Stripe has had so far.
async fn stalled_enterprise(domain: &str) -> (StripeStub, TestServer, TestOrg, TestUser, usize) {
    let stub = StripeStub::new(StubObjects::default()).await;
    let server = cloud(&stub, &LogCapture::default()).await;
    let admin = server
        .signup("Admin User", &format!("admin@admin.{domain}"))
        .await;
    let org = server.create_org(&admin, "Race Join Org").await;
    insert_metered_plan(&server, &org, "sub_race");
    bill(&stub, &org, "sub_race", "active", deal_item());
    let user = signed_up_before_sso(&server, &org, "Race User", domain).await;
    stub.delay(Duration::from_secs(1));
    let reads = stub.requests().len();
    (stub, server, org, user, reads)
}

/// Wait until Stripe has had more than `reads` requests, so the join is in its read of the plan,
/// and Stripe holds its answer back.
async fn plan_read_started(stub: &StripeStub, reads: usize) {
    tokio::time::timeout(ENTERPRISE_READ_TIMEOUT, async {
        while stub.requests().len() <= reads {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("The join never read the plan");
}

/// The user as the API reads them, for a join by SSO.
fn query_user(server: &TestServer, user: &TestUser) -> QueryUser {
    QueryUser::get_with_email(&mut server.db_conn(), &user.email).expect("Failed to get the user")
}

/// The roles the user holds in the organization.
fn roles(server: &TestServer, user: &TestUser, org: &TestOrg) -> Vec<String> {
    schema::organization_role::table
        .inner_join(schema::user::table)
        .inner_join(schema::organization::table)
        .filter(schema::user::uuid.eq(user.uuid))
        .filter(schema::organization::uuid.eq(org.uuid.to_string()))
        .select(schema::organization_role::role)
        .load(&mut server.db_conn())
        .expect("Failed to get the roles")
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
    let capture = LogCapture::default();
    let server = cloud(&stub, &capture).await;
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
    log_in(&server, &capture, &joined).await;
    assert!(member(&server, &joined, &enterprise), "a member stays");
    let late = server
        .signup("Late User", "late@enterprise.example.com")
        .await;
    assert!(!member(&server, &late, &enterprise), "no one new joins");
}

// Fails if an email sign-in fails, or joins the organization, when Bencher Cloud cannot read the
// plan of the organization at the user's SSO domain, or if it does not log why the user did not
// join.
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
    let user = signed_up_before_sso(&server, &org, "Broken User", "broken.example.com").await;

    log_in(&server, &capture, &user).await;

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

// Fails if an email sign-in, while Stripe stalls for 4 times the join's own limit, waits less than
// that limit or 1 second or more past it, fails, joins the organization, or does not log why the
// user did not join. Time is paused after the setup, so the limit and the stall pass in virtual
// time, in order.
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
    let user = signed_up_before_sso(&server, &org, "Stalled User", "stalled.example.com").await;
    stub.delay(ENTERPRISE_READ_TIMEOUT * 4);

    tokio::time::pause();
    let started = Instant::now();
    log_in(&server, &capture, &user).await;
    let waited = started.elapsed();
    tokio::time::resume();

    assert!(
        waited >= ENTERPRISE_READ_TIMEOUT,
        "the sign-in waited {waited:?}"
    );
    assert!(
        waited < ENTERPRISE_READ_TIMEOUT + Duration::from_secs(1),
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

// Fails if an email sign-in fails once it is in because the join by SSO after it fails, or if the
// user joins, or if the failure is not logged. A trigger in the test's database refuses the
// join's insert, as a database that fails it would.
#[tokio::test]
async fn sso_join_that_fails_never_fails_the_sign_in() {
    let stub = StripeStub::new(StubObjects::default()).await;
    let capture = LogCapture::default();
    let server = cloud(&stub, &capture).await;
    let admin = server
        .signup("Admin User", "refusedjoinadmin@example.com")
        .await;
    let org = server.create_org(&admin, "Refused Join Org").await;
    insert_metered_plan(&server, &org, "sub_refused");
    bill(&stub, &org, "sub_refused", "active", deal_item());
    let user = signed_up_before_sso(&server, &org, "Refused User", "refused.example.com").await;
    diesel::sql_query(format!(
        "CREATE TRIGGER refuse_join BEFORE INSERT ON organization_role \
         WHEN NEW.organization_id = (SELECT id FROM organization WHERE uuid = '{org}') \
         BEGIN SELECT RAISE(ABORT, 'refused by the test'); END",
        org = org.uuid,
    ))
    .execute(&mut server.db_conn())
    .expect("Failed to add the trigger");

    log_in(&server, &capture, &user).await;

    assert!(!member(&server, &user, &org));
    let uuid = user.uuid.to_string();
    assert!(
        capture
            .lines()
            .iter()
            .any(|line| line.contains(&uuid) && line.contains("without joining by SSO")),
        "{:?}",
        capture.lines()
    );
}

// Fails if the join by SSO still joins the user once the domain is removed while the join waits on
// Stripe: the join must check again, on the writer, that the domain still names the organization.
#[tokio::test]
async fn sso_join_skips_a_domain_removed_during_the_plan_read() {
    let domain = "removed.example.com";
    let (stub, server, org, user, reads) = stalled_enterprise(domain).await;
    let queried = query_user(&server, &user);
    tokio::time::pause();
    let join = Box::pin(QuerySso::join_domain(
        server.log(),
        server.context(),
        &queried,
        domain,
    ));
    let remove = async {
        plan_read_started(&stub, reads).await;
        diesel::delete(schema::sso::table.filter(schema::sso::domain.eq(domain)))
            .execute(&mut server.db_conn())
            .expect("Failed to remove the SSO domain");
    };
    let (joined, ()) = tokio::join!(join, remove);
    tokio::time::resume();

    assert!(matches!(joined, Ok(false)), "{joined:?}");
    assert!(!member(&server, &user, &org));
}

// Fails if the join by SSO still joins the user to the organization once its domain moves to
// another Enterprise organization while the join waits on Stripe, or joins them to that one, whose
// plan it did not read: the join must check again, on the writer, that the domain still names the
// organization whose plan it read.
#[tokio::test]
async fn sso_join_skips_a_domain_moved_during_the_plan_read() {
    let domain = "moved.example.com";
    let (stub, server, org, user, _) = stalled_enterprise(domain).await;
    let leader = server
        .signup("Other Leader", &format!("leader@admin.{domain}"))
        .await;
    let other = server.create_org(&leader, "Moved Join Org").await;
    insert_metered_plan(&server, &other, "sub_moved");
    bill(&stub, &other, "sub_moved", "active", deal_item());
    let reads = stub.requests().len();
    let queried = query_user(&server, &user);
    tokio::time::pause();
    let join = Box::pin(QuerySso::join_domain(
        server.log(),
        server.context(),
        &queried,
        domain,
    ));
    let move_domain = async {
        plan_read_started(&stub, reads).await;
        diesel::delete(schema::sso::table.filter(schema::sso::domain.eq(domain)))
            .execute(&mut server.db_conn())
            .expect("Failed to remove the SSO domain");
        insert_sso(&server, &other, domain);
    };
    let (joined, ()) = tokio::join!(join, move_domain);
    tokio::time::resume();

    assert!(matches!(joined, Ok(false)), "{joined:?}");
    assert!(!member(&server, &user, &org));
    assert!(!member(&server, &user, &other));
}

// Fails if a membership added while the join by SSO waits on Stripe makes the join answer an
// error, which a sign-in logs as a failed join, or changes the member's role: the join must check
// again, on the writer, that the user is still not a member.
#[tokio::test]
async fn sso_join_keeps_a_membership_added_during_the_plan_read() {
    let domain = "added.example.com";
    let (stub, server, org, user, reads) = stalled_enterprise(domain).await;
    let queried = query_user(&server, &user);
    tokio::time::pause();
    let join = Box::pin(QuerySso::join_domain(
        server.log(),
        server.context(),
        &queried,
        domain,
    ));
    let add = async {
        plan_read_started(&stub, reads).await;
        grant_organization_role(&server, &user, &org, Role::Member);
    };
    let (joined, ()) = tokio::join!(join, add);
    tokio::time::resume();

    assert!(matches!(joined, Ok(false)), "{joined:?}");
    assert_eq!(roles(&server, &user, &org), [Role::Member.to_string()]);
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
