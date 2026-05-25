#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::missing_assert_message,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args,
    reason = "integration test file"
)]
//! Integration tests for user API key authentication (`bencher_user_*`).
//!
//! These mirror `lib/api_projects/tests/project_key_auth.rs` but verify the
//! complementary property: a user-scoped key must authenticate as the owning
//! user across *every* endpoint a JWT can reach — organizations, projects,
//! user mgmt, token mgmt, even minting more keys.

use bencher_api_tests::TestServer;
use bencher_json::{
    JsonOrganization, JsonOrganizations, JsonProject, JsonToken, JsonUser, JsonUserKey,
    JsonUserKeyCreated, JsonUserKeys, UserKey,
};
use http::StatusCode;

async fn mint_key(server: &TestServer, user_slug: &str, token: &str, name: &str) -> UserKey {
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/users/{}/keys", user_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(token),
        )
        .json(&serde_json::json!({"name": name}))
        .send()
        .await
        .expect("Failed to create user key");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let created: JsonUserKeyCreated = resp.json().await.expect("Failed to parse new user key");
    created.key
}

async fn setup() -> (TestServer, String, String, UserKey) {
    let server = TestServer::new().await;
    let user = server.signup("Key User", "userkey-setup@example.com").await;
    let user_slug_ref: &str = user.slug.as_ref();
    let user_slug: String = user_slug_ref.to_owned();
    let key = mint_key(&server, &user_slug, &user.token, "primary-key").await;
    (server, user_slug, user.token, key)
}

// --- Positive cases: a user key is a drop-in JWT replacement -----------------

#[tokio::test]
async fn user_key_get_user() {
    let (server, user_slug, _token, key) = setup().await;

    let resp = server
        .client
        .get(server.api_url(&format!("/v0/users/{}", user_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let json_user: JsonUser = resp.json().await.expect("Failed to parse user");
    let slug: &str = json_user.slug.as_ref();
    assert_eq!(slug, user_slug);
}

#[tokio::test]
async fn user_key_list_organizations() {
    let (server, _user_slug, _token, key) = setup().await;

    let resp = server
        .client
        .get(server.api_url("/v0/organizations"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let orgs: JsonOrganizations = resp.json().await.expect("Failed to parse orgs");
    assert_eq!(orgs.0.len(), 1, "signup auto-creates a personal org");
}

#[tokio::test]
async fn user_key_create_organization() {
    let (server, _user_slug, _token, key) = setup().await;

    let resp = server
        .client
        .post(server.api_url("/v0/organizations"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .json(&serde_json::json!({"name": "Key-Created Org"}))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
    let org: JsonOrganization = resp.json().await.expect("Failed to parse org");
    let name: &str = org.name.as_ref();
    assert_eq!(name, "Key-Created Org");
}

#[tokio::test]
async fn user_key_create_project_on_personal_org() {
    let (server, _user_slug, _token, key) = setup().await;

    // Find personal org
    let orgs_resp = server
        .client
        .get(server.api_url("/v0/organizations"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .send()
        .await
        .expect("Request failed");
    let orgs: JsonOrganizations = orgs_resp.json().await.expect("orgs");
    let org_slug = &orgs.0[0].slug;
    let org_slug_str: &str = org_slug.as_ref();

    let resp = server
        .client
        .post(server.api_url(&format!("/v0/organizations/{}/projects", org_slug_str)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .json(&serde_json::json!({"name": "Key-Created Project"}))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
    let project: JsonProject = resp.json().await.expect("Failed to parse project");
    let name: &str = project.name.as_ref();
    assert_eq!(name, "Key-Created Project");
}

#[tokio::test]
async fn user_key_can_mint_another_user_key() {
    let (server, user_slug, _token, key) = setup().await;

    // Use the key (not the JWT) to mint a second key.
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/users/{}/keys", user_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .json(&serde_json::json!({"name": "second-key"}))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
    let created: JsonUserKeyCreated = resp.json().await.expect("Failed to parse second key");
    let name: &str = created.name.as_ref();
    assert_eq!(name, "second-key");
}

#[tokio::test]
async fn user_key_can_mint_user_jwt_token() {
    let (server, user_slug, _token, key) = setup().await;

    let resp = server
        .client
        .post(server.api_url(&format!("/v0/users/{}/tokens", user_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .json(&serde_json::json!({"name": "minted-by-key"}))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
    let _json: JsonToken = resp.json().await.expect("Failed to parse token");
}

#[tokio::test]
async fn user_key_list_keys() {
    let (server, user_slug, _token, key) = setup().await;

    let resp = server
        .client
        .get(server.api_url(&format!("/v0/users/{}/keys", user_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let keys: JsonUserKeys = resp.json().await.expect("Failed to parse keys");
    assert_eq!(keys.0.len(), 1);
}

// --- Negative cases: malformed / unknown / revoked ---------------------------

#[tokio::test]
async fn malformed_user_key_rejected() {
    let server = TestServer::new().await;
    // 12-char alphanumeric body — too short.
    let bogus = "bencher_user_AAAAAAAAAAAA";

    let resp = server
        .client
        .get(server.api_url("/v0/organizations"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(bogus),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn unknown_user_key_rejected() {
    let server = TestServer::new().await;
    // Valid format, no row in `user_key` table.
    let unknown = "bencher_user_aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh";

    let resp = server
        .client
        .get(server.api_url("/v0/organizations"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(unknown),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn revoked_user_key_rejected() {
    let (server, user_slug, token, key) = setup().await;

    // Discover the key's UUID via list.
    let list_resp = server
        .client
        .get(server.api_url(&format!("/v0/users/{}/keys", user_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&token),
        )
        .send()
        .await
        .expect("list failed");
    let keys: JsonUserKeys = list_resp.json().await.expect("list parse");
    let key_uuid = keys.0[0].uuid;

    // Revoke via the JWT (could just as well be the key itself).
    let del_resp = server
        .client
        .delete(server.api_url(&format!("/v0/users/{}/keys/{}", user_slug, key_uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&token),
        )
        .send()
        .await
        .expect("delete failed");
    assert_eq!(del_resp.status(), StatusCode::NO_CONTENT);

    // Try to reuse the now-revoked key.
    let resp = server
        .client
        .get(server.api_url("/v0/organizations"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn second_revoke_returns_conflict() {
    let (server, user_slug, token, _key) = setup().await;
    let list_resp = server
        .client
        .get(server.api_url(&format!("/v0/users/{}/keys", user_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&token),
        )
        .send()
        .await
        .expect("list failed");
    let keys: JsonUserKeys = list_resp.json().await.expect("list parse");
    let key_uuid = keys.0[0].uuid;

    for (i, expected) in [(0, StatusCode::NO_CONTENT), (1, StatusCode::CONFLICT)] {
        let resp = server
            .client
            .delete(server.api_url(&format!("/v0/users/{}/keys/{}", user_slug, key_uuid)))
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(&token),
            )
            .send()
            .await
            .expect("delete failed");
        assert_eq!(resp.status(), expected, "attempt {} status mismatch", i);
    }
}

// --- `same_user!` enforcement ------------------------------------------------

#[tokio::test]
async fn user_a_cannot_list_user_b_keys() {
    let server = TestServer::new().await;
    // First signup becomes admin; sacrifice an unused user so Alice and Bob are
    // both regular users and `same_user!` applies.
    let _admin = server.signup("Root", "root-list@example.com").await;
    let user_a = server.signup("Alice", "alice-keys@example.com").await;
    let user_b = server.signup("Bob", "bob-keys@example.com").await;
    let user_b_slug: &str = user_b.slug.as_ref();

    let resp = server
        .client
        .get(server.api_url(&format!("/v0/users/{}/keys", user_b_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user_a.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn user_a_cannot_create_user_b_key() {
    let server = TestServer::new().await;
    let _admin = server.signup("Root", "root-mint@example.com").await;
    let user_a = server.signup("Alice", "alice-mint@example.com").await;
    let user_b = server.signup("Bob", "bob-mint@example.com").await;
    let user_b_slug: &str = user_b.slug.as_ref();

    let resp = server
        .client
        .post(server.api_url(&format!("/v0/users/{}/keys", user_b_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user_a.token),
        )
        .json(&serde_json::json!({"name": "stolen"}))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn admin_can_list_user_b_keys() {
    let server = TestServer::new().await;
    // First signup becomes admin.
    let admin = server.signup("Root", "root-admin-list@example.com").await;
    let user_b = server.signup("Bob", "bob-admin-target@example.com").await;
    let user_b_slug: &str = user_b.slug.as_ref();

    let resp = server
        .client
        .get(server.api_url(&format!("/v0/users/{}/keys", user_b_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
}

// --- Hash uniqueness sanity --------------------------------------------------

#[tokio::test]
async fn distinct_keys_have_distinct_hashes() {
    let server = TestServer::new().await;
    let user = server.signup("Hash User", "hashuser@example.com").await;
    let user_slug: &str = user.slug.as_ref();

    let k1 = mint_key(&server, user_slug, &user.token, "one").await;
    let k2 = mint_key(&server, user_slug, &user.token, "two").await;
    assert_ne!(k1.as_ref(), k2.as_ref(), "generated keys must be unique");

    // Both should authenticate independently.
    for key in [k1, k2] {
        let resp = server
            .client
            .get(server.api_url("/v0/organizations"))
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(key.as_ref()),
            )
            .send()
            .await
            .expect("Request failed");
        assert_eq!(resp.status(), StatusCode::OK);
    }
}

// --- View / update --------------------------------------------------------

#[tokio::test]
async fn view_and_rename_user_key() {
    let (server, user_slug, token, _key) = setup().await;

    let list_resp = server
        .client
        .get(server.api_url(&format!("/v0/users/{}/keys", user_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&token),
        )
        .send()
        .await
        .expect("list failed");
    let keys: JsonUserKeys = list_resp.json().await.expect("list parse");
    let key_uuid = keys.0[0].uuid;

    // View
    let view_resp = server
        .client
        .get(server.api_url(&format!("/v0/users/{}/keys/{}", user_slug, key_uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&token),
        )
        .send()
        .await
        .expect("view failed");
    assert_eq!(view_resp.status(), StatusCode::OK);
    let key: JsonUserKey = view_resp.json().await.expect("view parse");
    let name: &str = key.name.as_ref();
    assert_eq!(name, "primary-key");

    // Rename
    let patch_resp = server
        .client
        .patch(server.api_url(&format!("/v0/users/{}/keys/{}", user_slug, key_uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&token),
        )
        .json(&serde_json::json!({"name": "renamed-key"}))
        .send()
        .await
        .expect("patch failed");
    assert_eq!(patch_resp.status(), StatusCode::OK);
    let patched: JsonUserKey = patch_resp.json().await.expect("patch parse");
    let name: &str = patched.name.as_ref();
    assert_eq!(name, "renamed-key");
}
