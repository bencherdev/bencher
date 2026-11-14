#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for auth accept (invite) endpoint.

use bencher_api_tests::{TestServer, TestUser};
use bencher_json::{Jwt, OrganizationUuid, organization::member::OrganizationRole};
use bencher_schema::{INVITE_TOKEN_TTL, schema};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use http::StatusCode;

// POST /v0/auth/accept - invalid invite token
#[tokio::test]
async fn accept_invalid_token() {
    let server = TestServer::new().await;

    let body = serde_json::json!({
        "invite": "invalid-token"
    });

    let resp = server
        .client
        .post(server.api_url("/v0/auth/accept"))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    // Invalid token should fail
    assert!(resp.status().is_client_error());
}

// POST /v0/auth/accept - empty invite token
#[tokio::test]
async fn accept_empty_token() {
    let server = TestServer::new().await;

    let body = serde_json::json!({
        "invite": ""
    });

    let resp = server
        .client
        .post(server.api_url("/v0/auth/accept"))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert!(resp.status().is_client_error());
}

// POST /v0/auth/accept - missing invite field
#[tokio::test]
async fn accept_missing_invite() {
    let server = TestServer::new().await;

    let body = serde_json::json!({});

    let resp = server
        .client
        .post(server.api_url("/v0/auth/accept"))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// POST /v0/auth/accept - an invite for another user's address, or for an address with no user, is
// refused, and the user joins nothing
#[tokio::test]
async fn accept_invite_for_another_address() {
    let server = TestServer::new().await;
    let leader = server.signup("Leader", "leader@example.com").await;
    let org = server.create_org(&leader, "Invited").await;
    let user = server.signup("User", "user@example.com").await;
    server.signup("Other", "other@example.com").await;

    for (invited, status) in [
        ("other@example.com", StatusCode::UNAUTHORIZED),
        ("nobody@example.com", StatusCode::NOT_FOUND),
    ] {
        let resp = accept(&server, &user, &invite(&server, invited, org.uuid)).await;
        assert_eq!(resp.status(), status, "{invited}");
    }
    assert_eq!(members(&server, org.uuid), ["leader@example.com"]);
}

// POST /v0/auth/accept - an invite the user already accepted is a conflict, and one to an
// organization deleted since is not found
#[tokio::test]
async fn accept_invite_that_no_longer_applies() {
    let server = TestServer::new().await;
    let leader = server.signup("Leader", "leader@example.com").await;
    let org = server.create_org(&leader, "Invited").await;
    let user = server.signup("User", "user@example.com").await;
    let other = server.signup("Other", "other@example.com").await;
    let user_invite = invite(&server, "user@example.com", org.uuid);
    let other_invite = invite(&server, "other@example.com", org.uuid);

    let resp = accept(&server, &user, &user_invite).await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let resp = accept(&server, &user, &user_invite).await;
    assert_eq!(resp.status(), StatusCode::CONFLICT);

    let resp = server
        .client
        .delete(server.api_url(&format!("/v0/organizations/{}", org.uuid)))
        .headers(server.fresh_session(&leader))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    let resp = accept(&server, &other, &other_invite).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        members(&server, org.uuid),
        ["leader@example.com", "user@example.com"]
    );
}

async fn accept(server: &TestServer, user: &TestUser, invite: &Jwt) -> reqwest::Response {
    server
        .client
        .post(server.api_url("/v0/auth/accept"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&serde_json::json!({ "invite": invite.as_ref() }))
        .send()
        .await
        .expect("Request failed")
}

fn invite(server: &TestServer, email: &str, organization: OrganizationUuid) -> Jwt {
    server
        .token_key()
        .new_invite(
            email.parse().expect("Invalid email"),
            INVITE_TOKEN_TTL,
            organization,
            OrganizationRole::Leader,
        )
        .expect("Failed to make an invite")
}

/// The email of every member of an organization, deleted or not, in the order they joined.
fn members(server: &TestServer, organization: OrganizationUuid) -> Vec<String> {
    schema::organization_role::table
        .inner_join(schema::organization::table)
        .inner_join(schema::user::table)
        .filter(schema::organization::uuid.eq(organization))
        .order(schema::organization_role::id)
        .select(schema::user::email)
        .load(&mut server.db_conn())
        .expect("Failed to load the members")
}
