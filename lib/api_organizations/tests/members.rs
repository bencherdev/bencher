#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::redundant_test_prefix,
    clippy::uninlined_format_args
)]
//! Integration tests for organization member endpoints.

use bencher_api_tests::TestServer;
use bencher_json::{
    JsonMembers,
    organization::member::{JsonNewMember, OrganizationRole},
};
use http::StatusCode;

// GET /v0/organizations/{organization}/members - list members
#[tokio::test]
async fn test_members_list() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "memberlist@example.com").await;
    let org = server.create_org(&user, "Members Org").await;

    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/organizations/{}/members", org_slug)))
        .header("Authorization", server.bearer(&user.token))
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
async fn test_members_creator_is_member() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "creatormember@example.com")
        .await;
    let org = server.create_org(&user, "Creator Member Org").await;

    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/organizations/{}/members", org_slug)))
        .header("Authorization", server.bearer(&user.token))
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
async fn test_members_invite() {
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
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    // Invitation sent
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
}

// POST /v0/organizations/{organization}/members - invite without name
#[tokio::test]
async fn test_members_invite_no_name() {
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
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    // Should still work without name
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
}

// POST /v0/organizations/{organization}/members - invalid email
#[tokio::test]
async fn test_members_invite_invalid_email() {
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
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}
