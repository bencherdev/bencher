//! Integration tests for organization endpoints.

use bencher_api_tests::TestServer;
use bencher_json::{
    JsonAllowed, JsonMembers, JsonNewOrganization, JsonOrganization, JsonOrganizations,
    organization::member::{JsonNewMember, OrganizationRole},
};
use http::StatusCode;

// GET /v0/organizations - requires auth
#[tokio::test]
async fn test_organizations_list_requires_auth() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .get(server.api_url("/v0/organizations"))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// GET /v0/organizations - authenticated
#[tokio::test]
async fn test_organizations_list() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "orgs@example.com").await;

    let resp = server
        .client
        .get(server.api_url("/v0/organizations"))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let orgs: JsonOrganizations = resp.json().await.expect("Failed to parse response");
    // User should have their personal org created during signup
    assert!(!orgs.0.is_empty());
}

// POST /v0/organizations - create new org
#[tokio::test]
async fn test_organizations_create() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "createorg@example.com").await;

    let body = JsonNewOrganization {
        name: "Test Organization".parse().unwrap(),
        slug: Some("test-organization".parse().unwrap()),
    };

    let resp = server
        .client
        .post(server.api_url("/v0/organizations"))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
    let org: JsonOrganization = resp.json().await.expect("Failed to parse response");
    assert_eq!(org.name.as_ref(), "Test Organization");
    let slug: &str = org.slug.as_ref();
    assert_eq!(slug, "test-organization");
}

// GET /v0/organizations/{organization} - view org
#[tokio::test]
async fn test_organizations_get() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "getorg@example.com").await;
    let org = server.create_org(&user, "View Org").await;

    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/organizations/{}", org_slug)))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let fetched: JsonOrganization = resp.json().await.expect("Failed to parse response");
    assert_eq!(fetched.uuid, org.uuid);
}

// PATCH /v0/organizations/{organization} - update org
#[tokio::test]
async fn test_organizations_update() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "updateorg@example.com").await;
    let org = server.create_org(&user, "Update Org").await;

    let body = serde_json::json!({
        "name": "Updated Org Name"
    });

    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .patch(server.api_url(&format!("/v0/organizations/{}", org_slug)))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let updated: JsonOrganization = resp.json().await.expect("Failed to parse response");
    assert_eq!(updated.name.as_ref(), "Updated Org Name");
}

// DELETE /v0/organizations/{organization} - delete org
#[tokio::test]
async fn test_organizations_delete() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "deleteorg@example.com").await;
    let org = server.create_org(&user, "Delete Org").await;

    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .delete(server.api_url(&format!("/v0/organizations/{}", org_slug)))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

// GET /v0/organizations/{organization}/allowed/{permission}
#[tokio::test]
async fn test_organizations_allowed() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "allowedorg@example.com").await;
    let org = server.create_org(&user, "Allowed Org").await;

    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/organizations/{}/allowed/view", org_slug)))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let allowed: JsonAllowed = resp.json().await.expect("Failed to parse response");
    assert!(allowed.allowed);
}

// GET /v0/organizations/{organization}/members
#[tokio::test]
async fn test_organizations_members_list() {
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

// POST /v0/organizations/{organization}/members - invite member
#[tokio::test]
async fn test_organizations_members_invite() {
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

// GET /v0/organizations/{organization}/projects
#[tokio::test]
async fn test_organizations_projects_list() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "orgprojects@example.com").await;
    let org = server.create_org(&user, "Projects Org").await;

    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/organizations/{}/projects", org_slug)))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
}

// POST /v0/organizations/{organization}/projects - create project
#[tokio::test]
async fn test_organizations_projects_create() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "createproject@example.com").await;
    let org = server.create_org(&user, "Create Project Org").await;

    let body = serde_json::json!({
        "name": "Test Project",
        "slug": "test-project"
    });

    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/organizations/{}/projects", org_slug)))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
}

// GET /v0/organizations/{organization}/plan - view plan
#[tokio::test]
async fn test_organizations_plan_get() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "planget@example.com").await;
    let org = server.create_org(&user, "Plan Org").await;

    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/organizations/{}/plan", org_slug)))
        .header("Authorization", server.bearer(&user.token))
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

// GET /v0/organizations/{organization}/usage - view usage
#[tokio::test]
async fn test_organizations_usage_get() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "usageget@example.com").await;
    let org = server.create_org(&user, "Usage Org").await;

    let org_slug: &str = org.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/organizations/{}/usage", org_slug)))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    // Usage endpoint may return NOT_FOUND if no usage data exists yet
    assert!(
        resp.status() == StatusCode::OK || resp.status() == StatusCode::NOT_FOUND,
        "Unexpected status: {}",
        resp.status()
    );
}
