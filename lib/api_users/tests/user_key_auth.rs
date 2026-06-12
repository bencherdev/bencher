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
//! user mgmt, token mgmt — except key management: a key cannot mint more user
//! keys and can only see, update, or revoke *itself*, so a leaked key cannot
//! outlive its own revocation, tamper with the user's other keys, or
//! enumerate them.

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
async fn user_key_cannot_mint_another_user_key() {
    let (server, user_slug, token, key) = setup().await;

    // A user key cannot mint another user key: a leaked key must not be able
    // to establish persistence that survives its own revocation.
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

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // No key was created: the JWT still sees only the original key.
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/users/{}/keys", user_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let keys: JsonUserKeys = resp.json().await.expect("Failed to parse keys");
    assert_eq!(keys.0.len(), 1);
}

// The tokens endpoint is deliberately out of scope for the mint restriction:
// tightening JWT API token semantics is a separate decision.
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

#[tokio::test]
async fn user_key_list_shows_only_itself() {
    let (server, user_slug, token, key) = setup().await;
    let _sibling = mint_key(&server, &user_slug, &token, "sibling-list").await;

    // The JWT sees both keys.
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/users/{}/keys", user_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("x-total-count")
            .and_then(|v| v.to_str().ok()),
        Some("2")
    );
    let keys: JsonUserKeys = resp.json().await.expect("Failed to parse keys");
    assert_eq!(keys.0.len(), 2);

    // The key sees only itself: sibling metadata (names, expirations) is
    // credential-inventory reconnaissance for a stolen key.
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
    assert_eq!(
        resp.headers()
            .get("x-total-count")
            .and_then(|v| v.to_str().ok()),
        Some("1")
    );
    let keys: JsonUserKeys = resp.json().await.expect("Failed to parse keys");
    assert_eq!(keys.0.len(), 1);
    let name: &str = keys.0[0].name.as_ref();
    assert_eq!(name, "primary-key");
}

#[tokio::test]
async fn user_key_can_view_itself() {
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

    let resp = server
        .client
        .get(server.api_url(&format!("/v0/users/{}/keys/{}", user_slug, key_uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .send()
        .await
        .expect("self-view failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let viewed: JsonUserKey = resp.json().await.expect("view parse");
    assert_eq!(viewed.uuid, key_uuid);
}

#[tokio::test]
async fn user_key_cannot_view_sibling_key() {
    let (server, user_slug, token, key) = setup().await;
    let _sibling = mint_key(&server, &user_slug, &token, "sibling-view").await;

    // Discover the sibling's UUID via list.
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
    let sibling_uuid = keys
        .0
        .iter()
        .find(|k| k.name.as_ref() == "sibling-view")
        .expect("sibling key should be listed")
        .uuid;

    let resp = server
        .client
        .get(server.api_url(&format!("/v0/users/{}/keys/{}", user_slug, sibling_uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .send()
        .await
        .expect("sibling view request failed");
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
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

#[tokio::test]
async fn user_key_can_revoke_itself() {
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

    // A key may always destroy itself: burn-after-use and in-CI incident
    // response must work without a JWT in hand.
    let resp = server
        .client
        .delete(server.api_url(&format!("/v0/users/{}/keys/{}", user_slug, key_uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .send()
        .await
        .expect("self-revoke failed");
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // The key no longer authenticates.
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
async fn user_key_cannot_revoke_sibling_key() {
    let (server, user_slug, token, key) = setup().await;
    let sibling = mint_key(&server, &user_slug, &token, "sibling-key").await;

    // Discover the sibling's UUID via list.
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
    let sibling_uuid = keys
        .0
        .iter()
        .find(|k| k.name.as_ref() == "sibling-key")
        .expect("sibling key should be listed")
        .uuid;

    // A key must not be able to revoke any key other than itself: a stolen key
    // revoking the user's other keys is a denial-of-service vector with no
    // legitimate workflow behind it.
    let resp = server
        .client
        .delete(server.api_url(&format!("/v0/users/{}/keys/{}", user_slug, sibling_uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .send()
        .await
        .expect("sibling revoke request failed");
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // The sibling key is untouched and still authenticates.
    let resp = server
        .client
        .get(server.api_url("/v0/organizations"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(sibling.as_ref()),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn user_key_can_rename_itself() {
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

    let resp = server
        .client
        .patch(server.api_url(&format!("/v0/users/{}/keys/{}", user_slug, key_uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .json(&serde_json::json!({"name": "renamed-by-self"}))
        .send()
        .await
        .expect("self-rename failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let renamed: JsonUserKey = resp.json().await.expect("rename parse");
    let name: &str = renamed.name.as_ref();
    assert_eq!(name, "renamed-by-self");
}

#[tokio::test]
async fn user_key_cannot_rename_sibling_key() {
    let (server, user_slug, token, key) = setup().await;
    let _sibling = mint_key(&server, &user_slug, &token, "sibling-rename").await;

    // Discover the sibling's UUID via list.
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
    let sibling_uuid = keys
        .0
        .iter()
        .find(|k| k.name.as_ref() == "sibling-rename")
        .expect("sibling key should be listed")
        .uuid;

    // A key may only mutate itself: renaming siblings could be abused to
    // obscure which key is which during an incident.
    let resp = server
        .client
        .patch(server.api_url(&format!("/v0/users/{}/keys/{}", user_slug, sibling_uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .json(&serde_json::json!({"name": "tampered"}))
        .send()
        .await
        .expect("sibling rename request failed");
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // The sibling key's name is unchanged.
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/users/{}/keys/{}", user_slug, sibling_uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&token),
        )
        .send()
        .await
        .expect("view failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let viewed: JsonUserKey = resp.json().await.expect("view parse");
    let name: &str = viewed.name.as_ref();
    assert_eq!(name, "sibling-rename");
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

// --- Expiration --------------------------------------------------------------

/// Mint a key with a short TTL, jump the injected clock past it, and confirm
/// the same key is rejected. Exercises the `expiration <= now` branch in
/// `user_from_user_key` (`auth.rs`) that the other negative tests don't reach.
#[cfg(feature = "plus")]
#[tokio::test]
async fn expired_user_key_rejected() {
    use std::sync::{
        Arc,
        atomic::{AtomicI64, Ordering},
    };

    let base_time = bencher_json::DateTime::TEST.timestamp();
    let mock_time = Arc::new(AtomicI64::new(base_time));
    let time_ref = mock_time.clone();
    let clock = bencher_json::Clock::Custom(Arc::new(move || {
        bencher_json::DateTime::try_from(time_ref.load(Ordering::Relaxed))
            .expect("Invalid mocked timestamp")
    }));

    let server = TestServer::new_with_clock(3600, 1024 * 1024, clock).await;
    let user = server
        .signup("Expiry User", "user-expiry@example.com")
        .await;
    let user_slug_ref: &str = user.slug.as_ref();
    let user_slug: String = user_slug_ref.to_owned();

    // Mint a key with a 60-second TTL.
    let create_resp = server
        .client
        .post(server.api_url(&format!("/v0/users/{}/keys", user_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&serde_json::json!({"name": "expiring-key", "ttl": 60}))
        .send()
        .await
        .expect("Failed to mint expiring key");
    assert_eq!(create_resp.status(), StatusCode::CREATED);
    let created: JsonUserKeyCreated = create_resp.json().await.expect("parse new key");
    let key = created.key;

    // Sanity: pre-expiry, the key authenticates.
    let pre_resp = server
        .client
        .get(server.api_url("/v0/organizations"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .send()
        .await
        .expect("pre-expiry request failed");
    assert_eq!(
        pre_resp.status(),
        StatusCode::OK,
        "key should work before expiration"
    );

    // Advance the clock past the TTL. `<= expiration` becomes true.
    mock_time.store(base_time + 120, Ordering::Relaxed);

    let post_resp = server
        .client
        .get(server.api_url("/v0/organizations"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .send()
        .await
        .expect("post-expiry request failed");
    assert_eq!(
        post_resp.status(),
        StatusCode::UNAUTHORIZED,
        "expired key should be rejected"
    );
}

// --- Locked user -------------------------------------------------------------

/// A key minted while the user was active must stop working once an admin locks
/// the user. Exercises the `query_user.locked` branch in `user_from_user_key`
/// (`auth.rs`) that the other negative tests don't reach.
#[tokio::test]
async fn locked_user_key_rejected() {
    let server = TestServer::new().await;
    // First signup becomes admin; only an admin can flip another user's `locked`.
    let admin = server.signup("Root", "root-lock@example.com").await;
    let user = server.signup("Lock User", "user-lock@example.com").await;
    let user_slug: &str = user.slug.as_ref();

    // Mint a key while the user is still active.
    let key = mint_key(&server, user_slug, &user.token, "lockable-key").await;

    // Sanity: pre-lock, the key authenticates.
    let pre_resp = server
        .client
        .get(server.api_url("/v0/organizations"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .send()
        .await
        .expect("pre-lock request failed");
    assert_eq!(
        pre_resp.status(),
        StatusCode::OK,
        "key should work before the user is locked"
    );

    // Admin locks the user.
    let lock_resp = server
        .client
        .patch(server.api_url(&format!("/v0/users/{}", user_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&admin.token),
        )
        .json(&serde_json::json!({"locked": true}))
        .send()
        .await
        .expect("lock request failed");
    assert_eq!(lock_resp.status(), StatusCode::OK);

    // The previously-valid key is now rejected via the `locked` branch.
    let post_resp = server
        .client
        .get(server.api_url("/v0/organizations"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .send()
        .await
        .expect("post-lock request failed");
    assert_eq!(
        post_resp.status(),
        StatusCode::UNAUTHORIZED,
        "locked user's key should be rejected"
    );
}
