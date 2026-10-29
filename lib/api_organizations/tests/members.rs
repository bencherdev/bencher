#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args,
    reason = "integration test file"
)]
//! Integration tests for organization member endpoints.

use bencher_api_tests::{TestOrg, TestServer, TestUser, helpers::user_organizations};
use bencher_json::{
    JsonMembers, OrganizationUuid, UserUuid,
    organization::member::{JsonNewMember, OrganizationRole},
    system::auth::JsonAccept,
};
use bencher_schema::INVITE_TOKEN_TTL;
use http::StatusCode;

// GET /v0/organizations/{organization}/members - list members
#[tokio::test]
async fn members_list() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "memberlist@example.com").await;
    let org = server.create_org(&user, "Members Org").await;

    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/organizations/{}/members", org_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let members: JsonMembers = resp.json().await.expect("Failed to parse response");
    // Creator should be a member
    assert!(!members.0.is_empty());
}

// GET /v0/organizations/{organization}/members - verify creator is member
#[tokio::test]
async fn members_creator_is_member() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "creatormember@example.com")
        .await;
    let org = server.create_org(&user, "Creator Member Org").await;

    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/organizations/{}/members", org_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let members: JsonMembers = resp.json().await.expect("Failed to parse response");

    // Find the creator in the members list
    let creator_found = members.0.iter().any(|m| m.uuid == user.uuid);
    assert!(creator_found, "Creator should be in members list");
}

// POST /v0/organizations/{organization}/members - invite member
#[tokio::test]
async fn members_invite() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "inviter@example.com").await;
    let org = server.create_org(&user, "Invite Org").await;

    let body = JsonNewMember {
        name: Some("Invitee".parse().unwrap()),
        email: "invitee@example.com".parse().unwrap(),
        role: OrganizationRole::Leader,
    };

    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/organizations/{}/members", org_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    // Invitation sent
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
}

// DELETE /v0/organizations/{organization}/members/{user} - a remove that leaves the member listed
// or takes them out of their other organizations
#[tokio::test]
async fn members_remove() {
    let server = TestServer::new().await;
    let leader = server.signup("Test User", "remover@example.com").await;
    let org = server.create_org(&leader, "Remove Org").await;
    let member = server.signup("Test Member", "removed@example.com").await;
    let invite = server
        .token_key()
        .new_invite(
            member.email.clone(),
            INVITE_TOKEN_TTL,
            org.uuid,
            OrganizationRole::Leader,
        )
        .expect("Failed to mint invite");
    let resp = server
        .client
        .post(server.api_url("/v0/auth/accept"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&member.token),
        )
        .json(&JsonAccept { invite })
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    assert!(
        member_uuids(&server, &leader, &org)
            .await
            .contains(&member.uuid)
    );

    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .delete(server.api_url(&format!(
            "/v0/organizations/{}/members/{}",
            org_slug, member.uuid
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&leader.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    assert_eq!(member_uuids(&server, &leader, &org).await, [leader.uuid]);
    assert_eq!(
        user_organizations(&server, member.uuid),
        [OrganizationUuid::from(member.uuid)]
    );
}

// POST /v0/organizations/{organization}/members - invite without name
#[tokio::test]
async fn members_invite_no_name() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "invitername@example.com").await;
    let org = server.create_org(&user, "No Name Invite Org").await;

    let body = JsonNewMember {
        name: None,
        email: "noname@example.com".parse().unwrap(),
        role: OrganizationRole::Leader,
    };

    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/organizations/{}/members", org_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    // Should still work without name
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
}

// POST /v0/organizations/{organization}/members - invalid email
#[tokio::test]
async fn members_invite_invalid_email() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "inviterbad@example.com").await;
    let org = server.create_org(&user, "Bad Email Invite Org").await;

    let body = serde_json::json!({
        "email": "not-an-email",
        "role": "leader"
    });

    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/organizations/{}/members", org_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[expect(clippy::expect_used, reason = "test helper listing members")]
async fn member_uuids(server: &TestServer, user: &TestUser, org: &TestOrg) -> Vec<UserUuid> {
    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/organizations/{}/members", org_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK, "Failed to list members");
    let members: JsonMembers = resp.json().await.expect("Failed to parse response");
    members.0.into_iter().map(|member| member.uuid).collect()
}
