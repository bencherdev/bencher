#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for the organization claim endpoint.

use bencher_api_tests::{
    TestServer, TestUser,
    helpers::{base_timestamp, user_organizations},
};
use bencher_json::{JsonNewClaim, JsonOrganization, OrganizationUuid};
use bencher_schema::{model::organization::InsertOrganization, schema};
use diesel::RunQueryDsl as _;
use http::StatusCode;

// POST /v0/organizations/{organization}/claim - a claim that leaves the user out of the organization
#[tokio::test]
async fn org_claim_unclaimed_organization() {
    let server = TestServer::new().await;
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
    let user = server.signup("Claim User", "claim@example.com").await;

    let (status, body) = post_claim(&server, &user, organization.uuid).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let claimed: JsonOrganization = serde_json::from_str(&body).expect("Failed to parse response");
    assert_eq!(claimed.uuid, organization.uuid);
    assert!(claimed.claimed.is_some());
    assert!(user_organizations(&server, user.uuid).contains(&organization.uuid));
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
