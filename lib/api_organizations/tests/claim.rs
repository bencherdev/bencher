#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for the organization claim endpoint.

use bencher_api_tests::{
    TestServer, TestUser,
    helpers::{base_timestamp, organization_roles, user_organizations},
};
use bencher_json::{
    JsonNewClaim, JsonOrganization, OrganizationUuid, organization::member::OrganizationRole,
};
use bencher_schema::{model::organization::InsertOrganization, schema};
use diesel::RunQueryDsl as _;
use http::StatusCode;

// POST /v0/organizations/{organization}/claim - a claim that leaves the user out of the organization
#[tokio::test]
async fn org_claim_unclaimed_organization() {
    let server = TestServer::new().await;
    let organization = insert_unclaimed_organization(&server);
    let user = server.signup("Claim User", "claim@example.com").await;

    let (status, body) = post_claim(&server, &user, organization).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let claimed: JsonOrganization = serde_json::from_str(&body).expect("Failed to parse response");
    assert_eq!(claimed.uuid, organization);
    assert!(claimed.claimed.is_some());
    assert!(user_organizations(&server, user.uuid).contains(&organization));
}

// POST /v0/organizations/{organization}/claim - a claim that takes over a claimed organization
#[tokio::test]
async fn org_claim_claimed_organization() {
    let server = TestServer::new().await;
    let leader = server.signup("Leader", "leader@example.com").await;
    let organization = server.create_org(&leader, "Claimed").await;
    let user = server.signup("Claim User", "claim@example.com").await;

    let (status, body) = post_claim(&server, &user, organization.uuid).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(
        user_organizations(&server, user.uuid),
        [OrganizationUuid::from(user.uuid)]
    );
}

// POST /v0/organizations/{organization}/claim - a claim by the Leader of an organization it claimed
#[tokio::test]
async fn org_claim_again_by_its_leader() {
    let server = TestServer::new().await;
    let organization = insert_unclaimed_organization(&server);
    let user = server.signup("Claim User", "claim@example.com").await;
    let (status, body) = post_claim(&server, &user, organization).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let (status, body) = post_claim(&server, &user, organization).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(
        organization_roles(&server, organization),
        [(user.uuid, OrganizationRole::Leader)]
    );
}

// POST /v0/organizations/{organization}/claim - concurrent claims by one user of an unclaimed
// organization, both queued on the writer before either claims it, both claim it, and the user
// holds the Leader role once
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn org_claim_at_once_by_a_user_claims_once() {
    let server = TestServer::new().await;
    let organization = insert_unclaimed_organization(&server);
    let user = server.signup("Claim User", "claim@example.com").await;
    let claim = || {
        server
            .client
            .post(server.api_url(&format!("/v0/organizations/{organization}/claim")))
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(&user.token),
            )
            .json(&JsonNewClaim { empty: None })
    };

    let responses = server
        .send_behind_the_writer(vec![vec![claim(), claim()]])
        .await;

    for (status, body) in responses {
        assert_eq!(status, StatusCode::CREATED, "{body}");
    }
    assert_eq!(
        organization_roles(&server, organization),
        [(user.uuid, OrganizationRole::Leader)]
    );
}

// POST /v0/organizations/{organization}/claim - two users race to claim an unclaimed organization,
// the first user's claim queued on the writer before the second's. The first user claims it, as its
// only Leader, and the second user's claim is refused, as it would be on the claimed organization.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn org_claim_at_once_by_two_users_claims_for_one() {
    let server = TestServer::new().await;
    let organization = insert_unclaimed_organization(&server);
    let first = server.signup("First User", "claimfirst@example.com").await;
    let second = server
        .signup("Second User", "claimsecond@example.com")
        .await;
    let claim = |user: &TestUser| {
        server
            .client
            .post(server.api_url(&format!("/v0/organizations/{organization}/claim")))
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(&user.token),
            )
            .json(&JsonNewClaim { empty: None })
    };

    let statuses = server
        .send_behind_the_writer(vec![vec![claim(&first)], vec![claim(&second)]])
        .await
        .into_iter()
        .map(|(status, _)| status)
        .collect::<Vec<_>>();

    assert_eq!(statuses, [StatusCode::CREATED, StatusCode::UNAUTHORIZED]);
    assert_eq!(
        organization_roles(&server, organization),
        [(first.uuid, OrganizationRole::Leader)]
    );
}

#[expect(clippy::expect_used, reason = "test helper inserting an organization")]
fn insert_unclaimed_organization(server: &TestServer) -> OrganizationUuid {
    let timestamp = base_timestamp();
    let organization = InsertOrganization {
        uuid: OrganizationUuid::new(),
        name: "Unclaimed".parse().expect("Invalid organization name"),
        slug: "unclaimed".parse().expect("Invalid organization slug"),
        created: timestamp,
        modified: timestamp,
    };
    diesel::insert_into(schema::organization::table)
        .values(&organization)
        .execute(&mut server.db_conn())
        .expect("Failed to insert organization");
    organization.uuid
}

#[expect(clippy::expect_used, reason = "test helper posting a claim")]
async fn post_claim(
    server: &TestServer,
    user: &TestUser,
    organization: OrganizationUuid,
) -> (StatusCode, String) {
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/organizations/{organization}/claim")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&JsonNewClaim { empty: None })
        .send()
        .await
        .expect("Request failed");
    let status = resp.status();
    (status, resp.text().await.expect("Failed to read response"))
}
