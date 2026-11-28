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

    // Without auth, should get an error (either 401 Unauthorized or 404 Not Found)
    assert!(
        resp.status() == StatusCode::UNAUTHORIZED || resp.status() == StatusCode::NOT_FOUND,
        "Expected 401 or 404, got: {}",
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
        .headers(server.fresh_session(&user))
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
        .headers(server.fresh_session(&user))
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::LOCKED);
}

// Fails if plan POST accepts a user API key or a deprecated user API token, or refuses a user
// session.
#[cfg(feature = "plus")]
#[tokio::test]
async fn plan_post_requires_session() {
    use bencher_api_tests::{bearer, helpers::seed_token};

    let server = TestServer::new_with_cloud_routes().await;
    let user = server
        .signup("Test User", "planpostsession@example.com")
        .await;
    let org = server.create_org(&user, "Plan Post Session Org").await;
    let api_token = seed_token(&server, &user, "Legacy Token");
    let key = server.create_user_key(&user, "plan-key").await;

    let org_slug: &str = org.slug.as_ref();
    let body = serde_json::json!({"checkout": "cs_test", "level": "pro"});
    let post = |credential| {
        server
            .client
            .post(server.api_url(&format!("/v0/organizations/{}/plan", org_slug)))
            .headers(credential)
            .json(&body)
            .send()
    };
    for credential in [bearer(api_token.token.as_ref()), bearer(key.as_ref())] {
        let resp = post(credential).await.expect("Request failed");
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    // A user session passes the gate and reaches the missing biller.
    let resp = post(server.fresh_session(&user))
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::LOCKED);
}

#[cfg(feature = "plus")]
#[expect(clippy::expect_used, reason = "test helpers")]
mod portal {
    use bencher_api_tests::{
        TestOrg, TestServer, TestUser, bearer,
        helpers::{base_timestamp, seed_token},
    };
    use bencher_schema::{model::user::UserId, schema};
    use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
    use http::{HeaderMap, StatusCode};

    async fn post_portal(server: &TestServer, org: &TestOrg, credential: HeaderMap) -> StatusCode {
        let org_slug: &str = org.slug.as_ref();
        server
            .client
            .post(server.api_url(&format!("/v0/organizations/{org_slug}/plan/portal")))
            .headers(credential)
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

    /// A metered plan row with no Stripe subscription behind it.
    fn insert_plan(server: &TestServer, org: &TestOrg) {
        let now = base_timestamp();
        diesel::insert_into(schema::plan::table)
            .values((
                schema::plan::organization_id.eq(organization_id(server, org)),
                schema::plan::metered_plan.eq("sub_portal"),
                schema::plan::created.eq(&now),
                schema::plan::modified.eq(&now),
            ))
            .execute(&mut server.db_conn())
            .expect("Failed to insert plan");
    }

    /// Make `user` a Member of `org`, a role without `manage`.
    fn insert_member(server: &TestServer, org: &TestOrg, user: &TestUser) {
        let mut conn = server.db_conn();
        let user_id: UserId = schema::user::table
            .filter(schema::user::uuid.eq(&user.uuid))
            .select(schema::user::id)
            .first(&mut conn)
            .expect("Failed to get user ID");
        let now = base_timestamp();
        diesel::insert_into(schema::organization_role::table)
            .values((
                schema::organization_role::user_id.eq(user_id),
                schema::organization_role::organization_id.eq(organization_id(server, org)),
                schema::organization_role::role.eq("member"),
                schema::organization_role::created.eq(&now),
                schema::organization_role::modified.eq(&now),
            ))
            .execute(&mut conn)
            .expect("Failed to insert organization role");
    }

    // Fails if the portal is served without Bencher Cloud's plan routes.
    #[tokio::test]
    async fn plan_portal_is_cloud_only() {
        let server = TestServer::new().await;
        let user = server.signup("Test User", "portalcloud@example.com").await;
        let org = server.create_org(&user, "Portal Cloud Org").await;
        insert_plan(&server, &org);

        assert_eq!(
            post_portal(&server, &org, server.fresh_session(&user)).await,
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
        insert_plan(&server, &org);
        let api_token = seed_token(&server, &user, "Legacy Token");
        assert_eq!(
            post_portal(&server, &org, bearer(api_token.token.as_ref())).await,
            StatusCode::FORBIDDEN
        );

        let user_slug: &str = user.slug.as_ref();
        let resp = server
            .client
            .post(server.api_url(&format!("/v0/users/{user_slug}/keys")))
            .headers(server.fresh_session(&user))
            .json(&serde_json::json!({"name": "portal-key"}))
            .send()
            .await
            .expect("Request failed");
        assert_eq!(resp.status(), StatusCode::CREATED);
        let created: bencher_json::JsonUserKeyCreated =
            resp.json().await.expect("Failed to parse user key");
        assert_eq!(
            post_portal(&server, &org, bearer(created.key.as_ref())).await,
            StatusCode::FORBIDDEN
        );
    }

    // Fails if the portal opens for anyone who cannot manage the organization.
    #[tokio::test]
    async fn plan_portal_requires_manage() {
        let server = TestServer::new_with_cloud_routes().await;
        let leader = server.signup("Test User", "portalleader@example.com").await;
        let org = server.create_org(&leader, "Portal Manage Org").await;
        insert_plan(&server, &org);
        let outsider = server
            .signup("Outsider", "portaloutsider@example.com")
            .await;
        assert_eq!(
            post_portal(&server, &org, server.fresh_session(&outsider)).await,
            StatusCode::FORBIDDEN
        );

        let member = server.signup("Member", "portalmember@example.com").await;
        insert_member(&server, &org, &member);
        assert_eq!(
            post_portal(&server, &org, server.fresh_session(&member)).await,
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
            post_portal(&server, &org, server.fresh_session(&user)).await,
            StatusCode::NOT_FOUND
        );

        insert_plan(&server, &org);
        // The test server has no biller, so a request that gets this far is refused there.
        assert_eq!(
            post_portal(&server, &org, server.fresh_session(&user)).await,
            StatusCode::LOCKED
        );
    }
}
