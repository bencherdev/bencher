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

#[cfg(feature = "plus")]
#[expect(clippy::expect_used, reason = "test helpers")]
mod portal {
    use bencher_api_tests::{
        TestOrg, TestServer,
        helpers::{base_timestamp, grant_organization_role, seed_token},
    };
    use bencher_rbac::organization::Role;
    use bencher_schema::schema;
    use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
    use http::StatusCode;

    async fn post_portal(server: &TestServer, org: &TestOrg, token: &str) -> StatusCode {
        let org_slug: &str = org.slug.as_ref();
        server
            .client
            .post(server.api_url(&format!("/v0/organizations/{org_slug}/plan/portal")))
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(token),
            )
            .send()
            .await
            .expect("Request failed")
            .status()
    }

    fn organization_id(server: &TestServer, org: &TestOrg) -> i32 {
        schema::organization::table
            .filter(schema::organization::uuid.eq(org.uuid.to_string()))
            .select(schema::organization::id)
            .first(&mut server.db_conn())
            .expect("Failed to get organization ID")
    }

    /// A metered plan row for `subscription`, with no Stripe subscription behind it.
    pub(super) fn insert_plan(server: &TestServer, org: &TestOrg, subscription: &str) {
        let now = base_timestamp();
        diesel::insert_into(schema::plan::table)
            .values((
                schema::plan::organization_id.eq(organization_id(server, org)),
                schema::plan::metered_plan.eq(subscription),
                schema::plan::created.eq(&now),
                schema::plan::modified.eq(&now),
            ))
            .execute(&mut server.db_conn())
            .expect("Failed to insert plan");
    }

    // Fails if the portal is served without Bencher Cloud's plan routes.
    #[tokio::test]
    async fn plan_portal_is_cloud_only() {
        let server = TestServer::new().await;
        let user = server.signup("Test User", "portalcloud@example.com").await;
        let org = server.create_org(&user, "Portal Cloud Org").await;
        insert_plan(&server, &org, "sub_portal");

        assert_eq!(
            post_portal(&server, &org, &user.token).await,
            StatusCode::NOT_FOUND
        );
    }

    // Fails if the portal opens for a user API key or a deprecated user API token.
    #[tokio::test]
    async fn plan_portal_requires_session() {
        let server = TestServer::new_with_cloud_routes().await;
        let user = server
            .signup("Test User", "portalsession@example.com")
            .await;
        let org = server.create_org(&user, "Portal Session Org").await;
        insert_plan(&server, &org, "sub_portal");
        let api_token = seed_token(&server, &user, "Legacy Token");
        assert_eq!(
            post_portal(&server, &org, api_token.token.as_ref()).await,
            StatusCode::FORBIDDEN
        );

        let user_slug: &str = user.slug.as_ref();
        let resp = server
            .client
            .post(server.api_url(&format!("/v0/users/{user_slug}/keys")))
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(&user.token),
            )
            .json(&serde_json::json!({"name": "portal-key"}))
            .send()
            .await
            .expect("Request failed");
        assert_eq!(resp.status(), StatusCode::CREATED);
        let created: bencher_json::JsonUserKeyCreated =
            resp.json().await.expect("Failed to parse user key");
        assert_eq!(
            post_portal(&server, &org, created.key.as_ref()).await,
            StatusCode::FORBIDDEN
        );
    }

    // Fails if the portal opens for anyone who cannot manage the organization.
    #[tokio::test]
    async fn plan_portal_requires_manage() {
        let server = TestServer::new_with_cloud_routes().await;
        let leader = server.signup("Test User", "portalleader@example.com").await;
        let org = server.create_org(&leader, "Portal Manage Org").await;
        insert_plan(&server, &org, "sub_portal");
        let outsider = server
            .signup("Outsider", "portaloutsider@example.com")
            .await;
        assert_eq!(
            post_portal(&server, &org, &outsider.token).await,
            StatusCode::FORBIDDEN
        );

        let member = server.signup("Member", "portalmember@example.com").await;
        grant_organization_role(&server, &member, &org, Role::Member);
        assert_eq!(
            post_portal(&server, &org, &member.token).await,
            StatusCode::FORBIDDEN
        );
    }

    // Fails if an organization without a plan reaches Stripe, or one with a plan never does.
    #[tokio::test]
    async fn plan_portal_needs_a_plan() {
        let server = TestServer::new_with_cloud_routes().await;
        let user = server.signup("Test User", "portalplan@example.com").await;
        let org = server.create_org(&user, "Portal Plan Org").await;
        assert_eq!(
            post_portal(&server, &org, &user.token).await,
            StatusCode::NOT_FOUND
        );

        insert_plan(&server, &org, "sub_portal");
        // The test server has no biller, so a request that gets this far is refused there.
        assert_eq!(
            post_portal(&server, &org, &user.token).await,
            StatusCode::LOCKED
        );
    }
}

// POST /v0/organizations/{organization}/plan takes a self-serve checkout return or a
// subscription that a server admin attaches. The test server has no biller, so a request
// that passes every check before Stripe is refused there with 423.
#[cfg(feature = "plus")]
#[expect(clippy::expect_used, reason = "test helpers")]
mod attach {
    use bencher_api_tests::{TestOrg, TestServer, TestUser, helpers::grant_organization_role};
    use bencher_rbac::organization::Role;
    use http::StatusCode;

    use super::portal::insert_plan;

    async fn post_plan(
        server: &TestServer,
        org: &TestOrg,
        user: &TestUser,
        body: &serde_json::Value,
    ) -> StatusCode {
        let org_slug: &str = org.slug.as_ref();
        server
            .client
            .post(server.api_url(&format!("/v0/organizations/{org_slug}/plan")))
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

    // Fails if anyone but a server admin attaches a subscription, even an organization's
    // leader, or a server admin's attach is refused before Stripe.
    #[tokio::test]
    async fn plan_attach_subscription_requires_admin() {
        let server = TestServer::new_with_cloud_routes().await;
        // The first user to sign up is the server admin.
        let admin = server.signup("Admin", "planattachadmin@example.com").await;
        let leader = server
            .signup("Leader", "planattachleader@example.com")
            .await;
        let org = server.create_org(&leader, "Plan Attach Org").await;
        let body = serde_json::json!({ "subscription": "sub_test" });

        assert_eq!(
            post_plan(&server, &org, &leader, &body).await,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            post_plan(&server, &org, &admin, &body).await,
            StatusCode::LOCKED
        );
    }

    // Fails if an attach reaches Stripe for a subscription that a plan row already holds,
    // since the attach would first name that subscription for another organization.
    #[tokio::test]
    async fn plan_attach_refuses_an_attached_subscription() {
        let server = TestServer::new_with_cloud_routes().await;
        let admin = server
            .signup("Admin", "planattachedadmin@example.com")
            .await;
        let holder = server.create_org(&admin, "Plan Holder Org").await;
        let org = server.create_org(&admin, "Plan Attach Again Org").await;
        insert_plan(&server, &holder, "sub_taken");

        assert_eq!(
            post_plan(
                &server,
                &org,
                &admin,
                &serde_json::json!({ "subscription": "sub_taken" })
            )
            .await,
            StatusCode::CONFLICT
        );
        assert_eq!(
            post_plan(
                &server,
                &org,
                &admin,
                &serde_json::json!({ "subscription": "sub_free" })
            )
            .await,
            StatusCode::LOCKED
        );
    }

    // Fails if a checkout return is taken from anyone who cannot manage the organization,
    // or refused from its leader before Stripe.
    #[tokio::test]
    async fn plan_checkout_requires_manage() {
        let server = TestServer::new_with_cloud_routes().await;
        let _admin = server
            .signup("Admin", "plancheckoutadmin@example.com")
            .await;
        let leader = server
            .signup("Leader", "plancheckoutleader@example.com")
            .await;
        let org = server.create_org(&leader, "Plan Checkout Org").await;
        let member = server
            .signup("Member", "plancheckoutmember@example.com")
            .await;
        grant_organization_role(&server, &member, &org, Role::Member);
        let outsider = server
            .signup("Outsider", "plancheckoutoutsider@example.com")
            .await;
        let body = serde_json::json!({ "checkout": "cs_test" });

        assert_eq!(
            post_plan(&server, &org, &outsider, &body).await,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            post_plan(&server, &org, &member, &body).await,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            post_plan(&server, &org, &leader, &body).await,
            StatusCode::LOCKED
        );
    }

    // Fails if a body that gives both a checkout and a subscription, or neither, or a checkout
    // that names a Self-Hosted organization, reaches Stripe, or a return from an earlier console
    // that still names its level, or an attach for a Self-Hosted organization, is refused.
    #[tokio::test]
    async fn plan_body_takes_one_of_checkout_or_subscription() {
        let server = TestServer::new_with_cloud_routes().await;
        let admin = server.signup("Admin", "planbodyadmin@example.com").await;
        let org = server.create_org(&admin, "Plan Body Org").await;
        let self_hosted = bencher_json::OrganizationUuid::new();

        for body in [
            serde_json::json!({ "checkout": "cs_test", "subscription": "sub_test" }),
            serde_json::json!({}),
            serde_json::json!({ "checkout": "" }),
            serde_json::json!({ "checkout": "cs_test", "self_hosted": self_hosted }),
            serde_json::json!({ "subscription": "sub_test", "self_hosted": "not-a-uuid" }),
        ] {
            assert_eq!(
                post_plan(&server, &org, &admin, &body).await,
                StatusCode::BAD_REQUEST,
                "{body}"
            );
        }
        for body in [
            serde_json::json!({ "checkout": "cs_test", "level": "pro" }),
            serde_json::json!({ "subscription": "sub_test", "self_hosted": self_hosted }),
        ] {
            assert_eq!(
                post_plan(&server, &org, &admin, &body).await,
                StatusCode::LOCKED,
                "{body}"
            );
        }
    }
}

// A Self-Hosted deal: a subscription on the Enterprise product with `series` metadata, which a
// server admin attaches to the Cloud organization that buys it, naming the Self-Hosted
// organization. Every Stripe request goes to the biller's stand-in.
#[cfg(feature = "plus")]
#[expect(clippy::expect_used, clippy::indexing_slicing, reason = "test helpers")]
mod self_hosted {
    use std::{collections::HashMap, sync::Arc};

    use bencher_api_tests::{
        LogCapture, TestOrg, TestServer, TestServerOptions, TestUser,
        license::{
            DAY, cloud_licensor, foreign_licensor, sign_earlier_license, sign_foreign_license,
            sign_license, stripe_cloud_with_licensor,
        },
        stripe_stub::{
            self as stub, ENTERPRISE_PRODUCT, NOW, PRO_PRODUCT, StripeStub, StubObjects,
        },
    };
    use bencher_json::{
        Clock, DateTime, JsonNewProject, JsonPlan, Jwt, OrganizationUuid, Priority,
        organization::usage::JsonUsage, project::Visibility,
    };
    use bencher_license::{GRACE_PERIOD, LicenseClaims, Licensor};
    use bencher_schema::{model::organization::QueryOrganization, schema};
    use diesel::{
        ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _, connection::SimpleConnection as _,
    };
    use http::StatusCode;
    use serde_json::{Value, json};

    use super::portal::insert_plan;

    /// A Self-Hosted deal for a flat fee a year, in a current period from `start` to `end`.
    fn deal(id: &str, status: &str, (start, end): (i64, i64), metadata: Value) -> Value {
        stub::subscription(
            id,
            status,
            "send_invoice",
            vec![stub::item_in(
                &stub::price("price_self_hosted", ENTERPRISE_PRODUCT, "1200000"),
                start,
                end,
            )],
            metadata,
            stub::customer(&json!({}), None),
            None,
        )
    }

    /// A deal whose plan item staff moved to a Pro price, in a current period from `start` to
    /// `end`.
    fn pro_deal(id: &str, status: &str, (start, end): (i64, i64), metadata: Value) -> Value {
        stub::subscription(
            id,
            status,
            "send_invoice",
            vec![stub::item_in(
                &stub::tiered_price(
                    "price_pro",
                    PRO_PRODUCT,
                    "volume",
                    &json!([{ "up_to": null, "flat_amount": 10_000 }]),
                ),
                start,
                end,
            )],
            metadata,
            stub::customer(&json!({}), None),
            None,
        )
    }

    /// The current period of every deal unless a test moves it: a day either side of `NOW`.
    const PERIOD: (i64, i64) = (NOW - DAY, NOW + DAY);

    async fn stripe(subscriptions: Vec<Value>) -> StripeStub {
        StripeStub::new(StubObjects {
            subscriptions: subscriptions
                .into_iter()
                .map(|subscription| {
                    let id = subscription["id"].as_str().expect("No id").to_owned();
                    (id, subscription)
                })
                .collect(),
            sessions: HashMap::new(),
        })
        .await
    }

    fn date(timestamp: i64) -> DateTime {
        DateTime::try_from(timestamp).expect("Invalid test time")
    }

    async fn send(user: &TestUser, request: reqwest::RequestBuilder) -> (StatusCode, String) {
        let resp = request
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(&user.token),
            )
            .send()
            .await
            .expect("Request failed");
        let status = resp.status();
        (status, resp.text().await.expect("Failed to read body"))
    }

    async fn attach(
        server: &TestServer,
        org: &TestOrg,
        user: &TestUser,
        body: &Value,
    ) -> (StatusCode, String) {
        let org_slug: &str = org.slug.as_ref();
        let request = server
            .client
            .post(server.api_url(&format!("/v0/organizations/{org_slug}/plan")))
            .json(body);
        send(user, request).await
    }

    /// Read `path` under the organization, `""` for the organization itself.
    async fn get(
        server: &TestServer,
        org: &TestOrg,
        user: &TestUser,
        path: &str,
    ) -> (StatusCode, String) {
        let org_slug: &str = org.slug.as_ref();
        let request = server
            .client
            .get(server.api_url(&format!("/v0/organizations/{org_slug}{path}")));
        send(user, request).await
    }

    /// The license key in the organization's plan, read through the API, which must show it.
    async fn plan_license(server: &TestServer, org: &TestOrg, user: &TestUser) -> Jwt {
        let (status, body) = get(server, org, user, "/plan").await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let plan: JsonPlan = serde_json::from_str(&body).expect("Invalid plan");
        plan.license.expect("No license").key
    }

    fn organization_id(server: &TestServer, org: &TestOrg) -> i32 {
        schema::organization::table
            .filter(schema::organization::uuid.eq(org.uuid.to_string()))
            .select(schema::organization::id)
            .first(&mut server.db_conn())
            .expect("Failed to get organization ID")
    }

    /// The organization's plan row: its metered plan, its licensed plan, and its license.
    fn plan_row(
        server: &TestServer,
        org: &TestOrg,
    ) -> (Option<String>, Option<String>, Option<String>) {
        schema::plan::table
            .filter(schema::plan::organization_id.eq(organization_id(server, org)))
            .select((
                schema::plan::metered_plan,
                schema::plan::licensed_plan,
                schema::plan::license,
            ))
            .first(&mut server.db_conn())
            .expect("Failed to get plan")
    }

    /// A licensed plan row for `subscription` that holds `license`, as an earlier attach left it.
    fn insert_licensed_plan(server: &TestServer, org: &TestOrg, subscription: &str, license: &Jwt) {
        let now = server.now();
        diesel::insert_into(schema::plan::table)
            .values((
                schema::plan::organization_id.eq(organization_id(server, org)),
                schema::plan::licensed_plan.eq(subscription),
                schema::plan::license.eq(license),
                schema::plan::created.eq(&now),
                schema::plan::modified.eq(&now),
            ))
            .execute(&mut server.db_conn())
            .expect("Failed to insert licensed plan");
    }

    /// A license that Bencher Cloud signed for `series` Benchmark Series on the Self-Hosted
    /// `organization`, for a term from `start` to `end`.
    fn license(organization: OrganizationUuid, series: u32, (start, end): (i64, i64)) -> Jwt {
        sign_license(organization, series, date(start), date(end))
    }

    fn claims(license: &Jwt) -> LicenseClaims {
        cloud_licensor().read(license).expect("Invalid license")
    }

    fn writes(stub: &StripeStub) -> usize {
        stub.requests()
            .into_iter()
            .filter(|request| request.method == "POST")
            .count()
    }

    /// The paths of every Stripe request so far.
    fn paths(stub: &StripeStub) -> Vec<String> {
        stub.requests()
            .into_iter()
            .map(|request| request.path)
            .collect()
    }

    // Fails if the attach of a Self-Hosted deal does not sign its first license for the
    // Self-Hosted organization, for the deal's `series`, issued at the start of the current
    // period with a term that ends with it and 30 days of grace after, or does not keep the
    // subscription and the license in the plan row, or gives the Cloud organization that buys it
    // a license or Enterprise.
    #[tokio::test]
    async fn self_hosted_attach_signs_the_first_license() {
        let stub = stripe(vec![deal(
            "sub_self_hosted",
            "active",
            PERIOD,
            json!({ "series": "500" }),
        )])
        .await;
        let server = TestServer::new_with_stripe(&stub).await;
        let admin = server.signup("Admin", "selfhostedsign@example.com").await;
        let org = server.create_org(&admin, "Self-Hosted Buyer Org").await;
        let self_hosted = OrganizationUuid::new();

        let (status, body) = attach(
            &server,
            &org,
            &admin,
            &json!({ "subscription": "sub_self_hosted", "self_hosted": self_hosted }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        let plan: JsonPlan = serde_json::from_str(&body).expect("Invalid plan");
        let license = plan.license.expect("No license");
        assert_eq!(license.organization, self_hosted);
        assert_eq!(u32::from(license.series), 500);
        assert_eq!(license.issued_at, date(PERIOD.0));
        assert_eq!(license.term_end, date(PERIOD.1));
        assert_eq!(license.expiration, date(PERIOD.1) + GRACE_PERIOD);
        let claims = claims(&license.key);
        assert_eq!(claims.sub, self_hosted);
        assert_eq!(u32::from(claims.ser), 500);
        assert_eq!(claims.iat, PERIOD.0);
        assert_eq!(claims.trm, PERIOD.1);
        assert_eq!(claims.exp, PERIOD.1 + GRACE_PERIOD.num_seconds());
        cloud_licensor()
            .validate_organization(&license.key, self_hosted, date(NOW))
            .expect("The license is not valid for the Self-Hosted organization");

        assert_eq!(
            plan_row(&server, &org),
            (
                None,
                Some("sub_self_hosted".to_owned()),
                Some(license.key.to_string())
            )
        );
        assert_eq!(
            stub.subscription("sub_self_hosted")["metadata"]["organization"],
            json!(org.uuid.to_string())
        );

        // The Cloud organization holds no license, and it is not Enterprise: a private project
        // is refused, as on Free, while a public one is made.
        let (status, body) = get(&server, &org, &admin, "").await;
        let org_json: Value = serde_json::from_str(&body).expect("Invalid organization");
        assert_eq!(status, StatusCode::OK, "{body}");
        assert!(org_json.get("license").is_none_or(Value::is_null), "{body}");
        let org_slug: &str = org.slug.as_ref();
        for (visibility, expected) in [
            (Visibility::Private, StatusCode::PAYMENT_REQUIRED),
            (Visibility::Public, StatusCode::CREATED),
        ] {
            let project = JsonNewProject {
                name: format!("Self-Hosted Buyer {visibility:?}")
                    .parse()
                    .expect("Invalid project name"),
                slug: None,
                url: None,
                visibility: Some(visibility),
            };
            let request = server
                .client
                .post(server.api_url(&format!("/v0/organizations/{org_slug}/projects")))
                .json(&project);
            let (status, body) = send(&admin, request).await;
            assert_eq!(status, expected, "{visibility:?}: {body}");
        }
    }

    // Fails if anyone but a server admin attaches a Self-Hosted deal, even the leader of the
    // organization that buys it, or a refusal reaches Stripe.
    #[tokio::test]
    async fn self_hosted_attach_requires_a_server_admin() {
        let stub = stripe(vec![deal(
            "sub_self_hosted",
            "active",
            PERIOD,
            json!({ "series": "500" }),
        )])
        .await;
        let server = TestServer::new_with_stripe(&stub).await;
        let admin = server.signup("Admin", "selfhostedadmin@example.com").await;
        let leader = server
            .signup("Leader", "selfhostedleader@example.com")
            .await;
        let org = server.create_org(&leader, "Self-Hosted Leader Org").await;
        let body =
            json!({ "subscription": "sub_self_hosted", "self_hosted": OrganizationUuid::new() });

        let (status, response) = attach(&server, &org, &leader, &body).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{response}");
        assert!(stub.requests().is_empty());

        let (status, response) = attach(&server, &org, &admin, &body).await;
        assert_eq!(status, StatusCode::CREATED, "{response}");
    }

    // Fails if an attach, for a Self-Hosted deal or for Bencher Cloud, reaches Stripe for a
    // subscription that a plan row already holds, as a Self-Hosted deal or as a Bencher Cloud plan,
    // or if a Self-Hosted deal attaches to an organization whose plan is still live, or reads or
    // writes the new subscription before it is refused.
    #[tokio::test]
    async fn self_hosted_attach_refuses_an_attached_subscription() {
        let series = json!({ "series": "500" });
        let stub = stripe(vec![
            deal("sub_free", "active", PERIOD, series.clone()),
            deal("sub_next", "active", PERIOD, series.clone()),
            deal("sub_licensed", "active", PERIOD, series.clone()),
            deal("sub_metered", "active", PERIOD, series),
        ])
        .await;
        let server = TestServer::new_with_stripe(&stub).await;
        let admin = server.signup("Admin", "selfhostedtaken@example.com").await;
        let licensed = server.create_org(&admin, "Self-Hosted Licensed Org").await;
        let metered = server.create_org(&admin, "Self-Hosted Metered Org").await;
        let org = server.create_org(&admin, "Self-Hosted Again Org").await;
        let self_hosted = OrganizationUuid::new();
        insert_licensed_plan(
            &server,
            &licensed,
            "sub_licensed",
            &license(self_hosted, 500, PERIOD),
        );
        insert_plan(&server, &metered, "sub_metered");

        for subscription in ["sub_licensed", "sub_metered"] {
            for body in [
                json!({ "subscription": subscription, "self_hosted": self_hosted }),
                json!({ "subscription": subscription }),
            ] {
                let (status, response) = attach(&server, &org, &admin, &body).await;
                assert_eq!(status, StatusCode::CONFLICT, "{body}: {response}");
            }
        }
        assert!(stub.requests().is_empty());

        // An organization whose plan is still live refuses another, having read only the status
        // of its own subscription.
        for (holder, held) in [(&licensed, "sub_licensed"), (&metered, "sub_metered")] {
            let (status, response) = attach(
                &server,
                holder,
                &admin,
                &json!({ "subscription": "sub_next", "self_hosted": OrganizationUuid::new() }),
            )
            .await;
            assert_eq!(status, StatusCode::CONFLICT, "{held}: {response}");
        }
        assert_eq!(
            paths(&stub),
            [
                "/v1/subscriptions/sub_licensed",
                "/v1/subscriptions/sub_metered"
            ]
        );

        let (status, response) = attach(
            &server,
            &org,
            &admin,
            &json!({ "subscription": "sub_free", "self_hosted": OrganizationUuid::new() }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{response}");
    }

    // Fails if a Pro subscription, a deal without a count in its `series` metadata, a canceled
    // deal, an incomplete deal, or a deal whose current period has already ended attaches as a
    // Self-Hosted deal, is refused with anything but a 400 that says why, or leaves a plan row or
    // a write to Stripe behind.
    #[tokio::test]
    async fn self_hosted_attach_refusals_say_why() {
        let pro = stub::subscription(
            "sub_pro",
            "active",
            "charge_automatically",
            vec![
                stub::item(&stub::tiered_price(
                    "price_pro",
                    PRO_PRODUCT,
                    "volume",
                    &json!([{ "up_to": null, "flat_amount": 10_000 }]),
                )),
                stub::item(&stub::price(
                    "price_bare_metal",
                    stub::BARE_METAL_PRODUCT,
                    "1.6666666667",
                )),
            ],
            json!({ "series": "500" }),
            stub::customer(&json!({}), None),
            None,
        );
        let stub = stripe(vec![
            pro,
            deal("sub_no_series", "active", PERIOD, json!({})),
            deal("sub_bad_series", "active", PERIOD, json!({ "series": "0" })),
            deal(
                "sub_canceled",
                "canceled",
                PERIOD,
                json!({ "series": "500" }),
            ),
            deal(
                "sub_ended",
                "active",
                (NOW, NOW),
                json!({ "series": "500" }),
            ),
            deal(
                "sub_incomplete",
                "incomplete",
                PERIOD,
                json!({ "series": "500" }),
            ),
        ])
        .await;
        let server = TestServer::new_with_stripe(&stub).await;
        let admin = server
            .signup("Admin", "selfhostedrefused@example.com")
            .await;
        let org = server.create_org(&admin, "Self-Hosted Refused Org").await;

        for (subscription, says) in [
            ("sub_pro", "Enterprise product"),
            ("sub_no_series", "`series` metadata"),
            ("sub_bad_series", "`series` metadata (0)"),
            ("sub_canceled", "is Canceled"),
            ("sub_ended", "already ended"),
            ("sub_incomplete", "is Incomplete"),
        ] {
            let (status, body) = attach(
                &server,
                &org,
                &admin,
                &json!({ "subscription": subscription, "self_hosted": OrganizationUuid::new() }),
            )
            .await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{subscription}: {body}");
            assert!(body.contains(says), "{subscription}: {body}");
        }
        assert_eq!(writes(&stub), 0);
        let (status, _) = get(&server, &org, &admin, "/plan").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    /// A server whose clock is frozen at `NOW`, the time in the stand-in's fixtures, with a
    /// Cloud organization that holds a licensed plan for `subscription` and `held`.
    async fn licensed_server(
        stub: &StripeStub,
        label: &str,
        subscription: &str,
        held: &Jwt,
    ) -> (TestServer, TestUser, TestOrg) {
        let (server, admin, org, _log) =
            logged_licensed_server(stub, label, subscription, held).await;
        (server, admin, org)
    }

    /// [`licensed_server`], with every line the server logs.
    async fn logged_licensed_server(
        stub: &StripeStub,
        label: &str,
        subscription: &str,
        held: &Jwt,
    ) -> (TestServer, TestUser, TestOrg, LogCapture) {
        logged_licensed_server_with_licensor(stub, cloud_licensor(), label, subscription, held)
            .await
    }

    /// [`logged_licensed_server`], whose Bencher Cloud signs licenses with `licensor`.
    async fn logged_licensed_server_with_licensor(
        stub: &StripeStub,
        licensor: Licensor,
        label: &str,
        subscription: &str,
        held: &Jwt,
    ) -> (TestServer, TestUser, TestOrg, LogCapture) {
        let now = date(NOW);
        let log = LogCapture::default();
        let server = TestServer::new_with_options(TestServerOptions {
            cloud_routes: true,
            clock: Some(Clock::Custom(Arc::new(move || now))),
            log: Some(log.logger()),
            cloud: Some(stripe_cloud_with_licensor(stub, licensor)),
            ..TestServerOptions::default()
        })
        .await;
        let admin = server
            .signup("Admin", &format!("selfhosted{label}@example.com"))
            .await;
        let org = server
            .create_org(&admin, &format!("Self-Hosted {label} Org"))
            .await;
        insert_licensed_plan(&server, &org, subscription, held);
        // Every write of a plan's license, counted, since a license signed again for the same
        // claims has the same key.
        server
            .db_conn()
            .batch_execute(
                "CREATE TABLE license_write (id INTEGER PRIMARY KEY);
                CREATE TRIGGER license_written AFTER UPDATE OF license ON plan
                BEGIN INSERT INTO license_write (id) VALUES (NULL); END;",
            )
            .expect("Failed to count license writes");
        (server, admin, org, log)
    }

    /// How many times a plan's license has been written since [`licensed_server`] made it.
    fn license_writes(server: &TestServer) -> i64 {
        diesel::dsl::sql::<diesel::sql_types::BigInt>("SELECT COUNT(*) FROM license_write")
            .get_result(&mut server.db_conn())
            .expect("Failed to count license writes")
    }

    // Fails if a read of a licensed plan whose subscription renewed does not sign and store a
    // license for the new term, for the same Self-Hosted organization and count, issued at the
    // start of the new period, signs again on the next read when nothing has changed since, or
    // writes the license of another plan.
    #[tokio::test]
    async fn licensed_plan_read_signs_a_renewal() {
        let self_hosted = OrganizationUuid::new();
        let held = license(self_hosted, 500, PERIOD);
        let renewed = (NOW + DAY, NOW + 366 * DAY);
        let stub = stripe(vec![deal("sub_renewed", "active", renewed, json!({}))]).await;
        let (server, admin, org) = licensed_server(&stub, "renewed", "sub_renewed", &held).await;
        let mut subscription = stub.subscription("sub_renewed");
        subscription["metadata"] = json!({ "series": "500", "organization": org.uuid.to_string() });
        stub.set_subscription(subscription);
        // Another organization's licensed plan, which the renewal leaves alone.
        let other = server
            .create_org(&admin, "Self-Hosted renewed Other Org")
            .await;
        let other_license = license(OrganizationUuid::new(), 250, PERIOD);
        insert_licensed_plan(&server, &other, "sub_other", &other_license);

        let key = plan_license(&server, &org, &admin).await;
        assert_ne!(key, held);
        let claims = claims(&key);
        assert_eq!(claims.sub, self_hosted);
        assert_eq!(u32::from(claims.ser), 500);
        assert_eq!(claims.iat, renewed.0);
        assert_eq!(claims.trm, renewed.1);
        assert_eq!(plan_row(&server, &org).2, Some(key.to_string()));
        assert_eq!(license_writes(&server), 1);

        assert_eq!(plan_license(&server, &org, &admin).await, key);
        assert_eq!(plan_row(&server, &org).2, Some(key.to_string()));
        assert_eq!(license_writes(&server), 1);
        assert_eq!(writes(&stub), 0);
        assert_eq!(plan_row(&server, &other).2, Some(other_license.to_string()));
    }

    // Fails if a read of a licensed plan whose `series` metadata changed in the term does not
    // sign and store a license for the new count with the same term.
    #[tokio::test]
    async fn licensed_plan_read_signs_an_upsell() {
        let self_hosted = OrganizationUuid::new();
        let held = license(self_hosted, 500, PERIOD);
        let stub = stripe(vec![deal("sub_upsold", "active", PERIOD, json!({}))]).await;
        let (server, admin, org) = licensed_server(&stub, "upsold", "sub_upsold", &held).await;
        let mut subscription = stub.subscription("sub_upsold");
        subscription["metadata"] = json!({ "series": "750", "organization": org.uuid.to_string() });
        stub.set_subscription(subscription);

        let key = plan_license(&server, &org, &admin).await;
        let claims = claims(&key);
        assert_eq!(claims.sub, self_hosted);
        assert_eq!(u32::from(claims.ser), 750);
        assert_eq!(claims.iat, PERIOD.0);
        assert_eq!(claims.trm, PERIOD.1);
        assert_eq!(plan_row(&server, &org).2, Some(key.to_string()));
        assert_eq!(license_writes(&server), 1);
    }

    // Fails if a read of a licensed plan signs a new license when nothing changed, when the
    // subscription is canceled or in another status that is not active though its period moved
    // on, or when its period ends before the term of the license held, whatever its count.
    #[tokio::test]
    async fn licensed_plan_read_keeps_the_license() {
        let self_hosted = OrganizationUuid::new();
        let held = license(self_hosted, 500, PERIOD);
        let renewed = (NOW + DAY, NOW + 366 * DAY);
        let earlier = (NOW - 2 * DAY, NOW);
        for (label, status, period, series) in [
            ("same", "active", PERIOD, "500"),
            ("canceled", "canceled", renewed, "500"),
            ("pastdue", "past_due", renewed, "500"),
            ("unpaid", "unpaid", renewed, "750"),
            ("paused", "paused", renewed, "500"),
            ("earlier", "active", earlier, "750"),
        ] {
            let stub = stripe(vec![deal("sub_kept", status, period, json!({}))]).await;
            let (server, admin, org) = licensed_server(&stub, label, "sub_kept", &held).await;
            let mut subscription = stub.subscription("sub_kept");
            subscription["metadata"] =
                json!({ "series": series, "organization": org.uuid.to_string() });
            stub.set_subscription(subscription);

            assert_eq!(plan_license(&server, &org, &admin).await, held, "{label}");
            assert_eq!(plan_row(&server, &org).2, Some(held.to_string()), "{label}");
            assert_eq!(license_writes(&server), 0, "{label}");
        }
    }

    // Fails if two reads at once of a licensed plan whose subscription renewed store anything but
    // one valid license for the new term, or show anything but that license, or both write it
    // when they overlap.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn licensed_plan_reads_at_once_store_one_license() {
        let self_hosted = OrganizationUuid::new();
        let held = license(self_hosted, 500, PERIOD);
        let renewed = (NOW + DAY, NOW + 366 * DAY);
        let stub = stripe(vec![deal("sub_twice", "active", renewed, json!({}))]).await;
        let (server, admin, org) = licensed_server(&stub, "twice", "sub_twice", &held).await;
        let mut subscription = stub.subscription("sub_twice");
        subscription["metadata"] = json!({ "series": "500", "organization": org.uuid.to_string() });
        stub.set_subscription(subscription);

        let (first, second) = tokio::join!(
            plan_license(&server, &org, &admin),
            plan_license(&server, &org, &admin)
        );
        assert_eq!(first, second);
        let stored = plan_row(&server, &org).2.expect("No license");
        assert_eq!(stored, first.to_string());
        let claims = claims(&first);
        assert_eq!(claims.trm, renewed.1);
        cloud_licensor()
            .validate_organization(&first, self_hosted, date(NOW + 2 * DAY))
            .expect("The stored license is not valid");
        assert_eq!(license_writes(&server), 1);
    }

    // Fails if a license that Bencher Cloud keeps fails the plan or the usage read once it has
    // expired, as one does after its subscription is canceled, or if the usage does not end at
    // the server's time, by which the console tells an expired license from one in its grace.
    #[tokio::test]
    async fn an_expired_license_on_cloud_still_renders() {
        let self_hosted = OrganizationUuid::new();
        let ended = (NOW - 400 * DAY, NOW - 365 * DAY);
        let held = license(self_hosted, 500, ended);
        let stub = stripe(vec![deal("sub_expired", "canceled", ended, json!({}))]).await;
        let (server, admin, org) = licensed_server(&stub, "expired", "sub_expired", &held).await;
        let mut subscription = stub.subscription("sub_expired");
        subscription["metadata"] = json!({ "series": "500", "organization": org.uuid.to_string() });
        stub.set_subscription(subscription);

        let (status, body) = get(&server, &org, &admin, "/plan").await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let plan: JsonPlan = serde_json::from_str(&body).expect("Invalid plan");
        let license = plan.license.expect("No license");
        assert_eq!(license.key, held);
        assert_eq!(license.term_end, date(ended.1));
        assert_eq!(license.expiration, date(ended.1) + GRACE_PERIOD);

        let (status, body) = get(&server, &org, &admin, "/usage").await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let usage: JsonUsage = serde_json::from_str(&body).expect("Invalid usage");
        assert_eq!(
            serde_json::to_value(usage.kind).expect("Invalid kind"),
            json!("cloud_self_hosted_licensed")
        );
        assert_eq!(usage.license.map(|license| license.key), Some(held));
        assert_eq!(usage.start_time, date(ended.0));
        assert_eq!(usage.end_time, date(NOW));
        assert_eq!(license_writes(&server), 0);
    }

    // Fails if a read of a licensed plan whose active subscription cannot be licensed now, for a
    // period that has already ended, without a count in its `series` metadata, or off the
    // Enterprise product, fails the plan or the usage read, changes the license held, or does not
    // say why in the log, or if a canceled one says anything.
    #[tokio::test]
    async fn licensed_plan_read_keeps_the_license_it_cannot_renew() {
        let self_hosted = OrganizationUuid::new();
        let held = license(self_hosted, 500, (NOW - 40 * DAY, NOW - 10 * DAY));
        let renewed = (NOW, NOW + 30 * DAY);
        for (label, subscription, series, says) in [
            (
                "ended",
                deal("sub_kept", "active", (NOW, NOW - DAY), json!({})),
                Some("500"),
                Some("already ended"),
            ),
            (
                "unseries",
                deal("sub_kept", "active", renewed, json!({})),
                None,
                Some("no `series` metadata"),
            ),
            (
                "badseries",
                deal("sub_kept", "active", renewed, json!({})),
                Some("1,500"),
                Some("`series` metadata (1,500)"),
            ),
            (
                "pro",
                pro_deal("sub_kept", "active", renewed, json!({})),
                Some("500"),
                Some("Enterprise product"),
            ),
            (
                "lapsed",
                deal("sub_kept", "canceled", (NOW, NOW - DAY), json!({})),
                Some("500"),
                None,
            ),
        ] {
            let stub = stripe(vec![subscription]).await;
            let (server, admin, org, log) =
                logged_licensed_server(&stub, label, "sub_kept", &held).await;
            let mut subscription = stub.subscription("sub_kept");
            subscription["metadata"] = json!({ "organization": org.uuid.to_string() });
            if let Some(series) = series {
                subscription["metadata"]["series"] = json!(series);
            }
            stub.set_subscription(subscription);

            assert_eq!(plan_license(&server, &org, &admin).await, held, "{label}");
            let (status, body) = get(&server, &org, &admin, "/usage").await;
            assert_eq!(status, StatusCode::OK, "{label}: {body}");
            let usage: JsonUsage = serde_json::from_str(&body).expect("Invalid usage");
            assert_eq!(
                usage.license.map(|license| license.key),
                Some(held.clone()),
                "{label}"
            );
            assert_eq!(plan_row(&server, &org).2, Some(held.to_string()), "{label}");
            assert_eq!(license_writes(&server), 0, "{label}");
            let kept = log
                .lines()
                .into_iter()
                .filter(|line| line.contains("signed no new license"))
                .collect::<Vec<_>>();
            if let Some(says) = says {
                assert!(
                    !kept.is_empty() && kept.iter().all(|line| line.contains(says)),
                    "{label}: {kept:?}"
                );
            } else {
                assert!(kept.is_empty(), "{label}: {kept:?}");
            }
        }
    }

    // Fails if a license that Bencher Cloud keeps, whose key no longer verifies, as after the
    // signing key changes, is not signed again for its own Self-Hosted organization while its
    // subscription is active, with the count and the term that the subscription pays for, not
    // the ones in the license, or fails the plan or the usage read.
    #[tokio::test]
    async fn a_license_that_no_longer_verifies_is_signed_again() {
        let self_hosted = OrganizationUuid::new();
        let held = sign_foreign_license(self_hosted, 250, date(NOW - 40 * DAY), date(PERIOD.1));
        cloud_licensor().read(&held).unwrap_err();
        let stub = stripe(vec![deal("sub_rotated", "active", PERIOD, json!({}))]).await;
        let (server, admin, org) = licensed_server(&stub, "rotated", "sub_rotated", &held).await;
        let mut subscription = stub.subscription("sub_rotated");
        subscription["metadata"] = json!({ "series": "500", "organization": org.uuid.to_string() });
        stub.set_subscription(subscription);

        let key = plan_license(&server, &org, &admin).await;
        let claims = claims(&key);
        assert_eq!(claims.sub, self_hosted);
        assert_eq!(u32::from(claims.ser), 500);
        assert_eq!(claims.iat, PERIOD.0);
        assert_eq!(claims.trm, PERIOD.1);
        assert_eq!(plan_row(&server, &org).2, Some(key.to_string()));
        assert_eq!(license_writes(&server), 1);

        let (status, body) = get(&server, &org, &admin, "/usage").await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let usage: JsonUsage = serde_json::from_str(&body).expect("Invalid usage");
        assert_eq!(usage.license.map(|license| license.key), Some(key));
        assert_eq!(license_writes(&server), 1);
    }

    // Fails if a read that signs a new license with a private key that does not match the public
    // key stores it, fails the plan or the usage read, shows anything but the license held when
    // that one verifies and no license when it does not, or does not say why in the log.
    #[tokio::test]
    async fn a_new_license_that_does_not_verify_is_not_stored() {
        let self_hosted = OrganizationUuid::new();
        let renewed = (NOW + DAY, NOW + 366 * DAY);
        for (label, held, shown) in [
            ("mismatchrenewed", license(self_hosted, 500, PERIOD), true),
            (
                "mismatchrotated",
                sign_foreign_license(self_hosted, 500, date(PERIOD.0), date(PERIOD.1)),
                false,
            ),
        ] {
            let stub = stripe(vec![deal("sub_mismatch", "active", renewed, json!({}))]).await;
            let (server, admin, org, log) = logged_licensed_server_with_licensor(
                &stub,
                foreign_licensor(),
                label,
                "sub_mismatch",
                &held,
            )
            .await;
            let mut subscription = stub.subscription("sub_mismatch");
            subscription["metadata"] =
                json!({ "series": "500", "organization": org.uuid.to_string() });
            stub.set_subscription(subscription);
            let shown = shown.then_some(held.clone());

            let (status, body) = get(&server, &org, &admin, "/plan").await;
            assert_eq!(status, StatusCode::OK, "{label}: {body}");
            let plan: JsonPlan = serde_json::from_str(&body).expect("Invalid plan");
            assert_eq!(plan.license.map(|license| license.key), shown, "{label}");

            let (status, body) = get(&server, &org, &admin, "/usage").await;
            assert_eq!(status, StatusCode::OK, "{label}: {body}");
            let usage: JsonUsage = serde_json::from_str(&body).expect("Invalid usage");
            assert_eq!(usage.license.map(|license| license.key), shown, "{label}");

            assert_eq!(plan_row(&server, &org).2, Some(held.to_string()), "{label}");
            assert_eq!(license_writes(&server), 0, "{label}");
            assert!(
                log.lines()
                    .iter()
                    .any(|line| line.contains("The new license does not verify")),
                "{label}"
            );
        }
    }

    // Fails if a license whose key no longer verifies, and that a read cannot sign again since
    // its subscription is canceled, pays for an earlier term, or has no count in its `series`
    // metadata, or since the license names no organization or term to sign for, fails the plan
    // or the usage read rather than leaving the license out, is written over, is not named in
    // the log, or is said in the log to stay.
    #[tokio::test]
    async fn a_license_that_no_longer_verifies_and_is_not_signed_again_is_left_out() {
        let self_hosted = OrganizationUuid::new();
        let foreign = |(start, end)| sign_foreign_license(self_hosted, 500, date(start), date(end));
        for (label, status, held, series) in [
            ("rotatedcanceled", "canceled", foreign(PERIOD), Some("500")),
            (
                "rotatedearlier",
                "active",
                foreign((NOW, NOW + 400 * DAY)),
                Some("500"),
            ),
            ("rotatedunseries", "active", foreign(PERIOD), None),
            // A key with no organization or term to sign again for.
            (
                "earlierformat",
                "active",
                sign_earlier_license(self_hosted, date(PERIOD.0), date(PERIOD.1)),
                Some("500"),
            ),
        ] {
            let stub = stripe(vec![deal("sub_left", status, PERIOD, json!({}))]).await;
            let (server, admin, org, log) =
                logged_licensed_server(&stub, label, "sub_left", &held).await;
            let mut subscription = stub.subscription("sub_left");
            subscription["metadata"] = json!({ "organization": org.uuid.to_string() });
            if let Some(series) = series {
                subscription["metadata"]["series"] = json!(series);
            }
            stub.set_subscription(subscription);

            let (status, body) = get(&server, &org, &admin, "/plan").await;
            assert_eq!(status, StatusCode::OK, "{label}: {body}");
            let plan: JsonPlan = serde_json::from_str(&body).expect("Invalid plan");
            assert!(plan.license.is_none(), "{label}: {body}");

            let (status, body) = get(&server, &org, &admin, "/usage").await;
            assert_eq!(status, StatusCode::OK, "{label}: {body}");
            let usage: JsonUsage = serde_json::from_str(&body).expect("Invalid usage");
            assert_eq!(
                serde_json::to_value(usage.kind).expect("Invalid kind"),
                json!("cloud_self_hosted_licensed")
            );
            assert!(usage.license.is_none(), "{label}: {body}");
            assert_eq!(usage.end_time, date(NOW), "{label}");

            assert_eq!(plan_row(&server, &org).2, Some(held.to_string()), "{label}");
            assert_eq!(license_writes(&server), 0, "{label}");
            let lines = log.lines();
            assert!(
                lines.iter().any(|line| line.contains("does not verify")),
                "{label}"
            );
            let unsigned = lines
                .iter()
                .filter(|line| line.contains("signed no new license"))
                .collect::<Vec<_>>();
            if series.is_some() {
                assert!(unsigned.is_empty(), "{label}: {unsigned:?}");
            } else {
                assert!(
                    !unsigned.is_empty()
                        && unsigned.iter().all(|line| {
                            line.contains("signed no new license, and the plan shows none")
                                && line.contains("no `series` metadata")
                        }),
                    "{label}: {unsigned:?}"
                );
            }
        }
    }

    // Fails if a licensed plan whose subscription has ended blocks the next attach, which staff
    // make when they sell the next contract as a new subscription, or the next attach keeps the
    // old plan row.
    #[tokio::test]
    async fn a_licensed_plan_whose_subscription_ended_gives_way() {
        let series = json!({ "series": "500" });
        let stub = stripe(vec![
            deal("sub_old", "canceled", PERIOD, series.clone()),
            deal("sub_new", "active", PERIOD, series),
        ])
        .await;
        let server = TestServer::new_with_stripe(&stub).await;
        let admin = server.signup("Admin", "selfhostednext@example.com").await;
        let org = server.create_org(&admin, "Self-Hosted Next Org").await;
        let self_hosted = OrganizationUuid::new();
        insert_licensed_plan(&server, &org, "sub_old", &license(self_hosted, 250, PERIOD));

        let (status, body) = attach(
            &server,
            &org,
            &admin,
            &json!({ "subscription": "sub_new", "self_hosted": self_hosted }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        let plan: JsonPlan = serde_json::from_str(&body).expect("Invalid plan");
        let key = plan.license.expect("No license").key;
        assert_eq!(
            plan_row(&server, &org),
            (None, Some("sub_new".to_owned()), Some(key.to_string()))
        );
    }

    /// The registry priority of `org`, as a registry transfer reads it.
    fn priority(server: &TestServer, org: &TestOrg) -> Priority {
        let mut conn = server.db_conn();
        QueryOrganization::from_uuid(&mut conn, org.uuid)
            .expect("Failed to get organization")
            .oci_bandwidth_priority(&mut conn, &server.context().licensor, server.now())
            .expect("Failed to read priority")
    }

    // Fails if the Bencher Cloud organization that buys a Self-Hosted license gets a paid
    // organization's registry priority from its licensed plan row, or one with a Bencher Cloud
    // plan row does not.
    #[tokio::test]
    async fn a_licensed_plan_row_gives_no_registry_priority() {
        let server = TestServer::new_with_cloud_routes().await;
        let admin = server
            .signup("Admin", "selfhostedpriority@example.com")
            .await;
        let free = server
            .create_org(&admin, "Self-Hosted Free Priority Org")
            .await;
        let licensed = server
            .create_org(&admin, "Self-Hosted Licensed Priority Org")
            .await;
        let metered = server
            .create_org(&admin, "Self-Hosted Metered Priority Org")
            .await;
        insert_licensed_plan(
            &server,
            &licensed,
            "sub_licensed",
            &license(OrganizationUuid::new(), 500, PERIOD),
        );
        insert_plan(&server, &metered, "sub_metered");

        assert_eq!(priority(&server, &free), Priority::Free);
        assert_eq!(priority(&server, &licensed), Priority::Free);
        assert_eq!(priority(&server, &metered), Priority::Plus);
    }

    // Fails if the organization the first license is signed for reads back as another, or the
    // time it is issued moves, between the attach and the next read.
    #[tokio::test]
    async fn self_hosted_attach_then_read_shows_the_same_license() {
        let stub = stripe(vec![deal(
            "sub_then",
            "active",
            PERIOD,
            json!({ "series": "500" }),
        )])
        .await;
        let server = TestServer::new_with_stripe(&stub).await;
        let admin = server.signup("Admin", "selfhostedthen@example.com").await;
        let org = server.create_org(&admin, "Self-Hosted Then Org").await;
        let self_hosted = OrganizationUuid::new();
        let (status, body) = attach(
            &server,
            &org,
            &admin,
            &json!({ "subscription": "sub_then", "self_hosted": self_hosted }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        let plan: JsonPlan = serde_json::from_str(&body).expect("Invalid plan");
        let attached = plan.license.expect("No license").key;

        assert_eq!(plan_license(&server, &org, &admin).await, attached);
    }
}
