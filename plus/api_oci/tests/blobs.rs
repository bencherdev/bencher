#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args,
    clippy::redundant_test_prefix
)]
//! Integration tests for OCI blob endpoints.

use bencher_api_tests::TestServer;
use bencher_token::OciAction;
use http::StatusCode;
use sha2::{Digest as _, Sha256};

/// Helper to compute SHA256 digest in OCI format
fn compute_digest(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let hash = hasher.finalize();
    format!("sha256:{}", hex::encode(hash))
}

// POST /v2/{name}/blobs/uploads - Start upload (authenticated)
#[tokio::test]
async fn test_blob_upload_start() {
    let server = TestServer::new().await;
    let user = server.signup("Blob User", "blobstart@example.com").await;
    let org = server.create_org(&user, "Blob Org").await;
    let project = server.create_project(&user, &org, "Blob Project").await;

    let oci_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    let resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", project_slug)))
        .header("Authorization", format!("Bearer {}", oci_token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    assert!(resp.headers().contains_key("location"));
    assert!(resp.headers().contains_key("docker-upload-uuid"));
}

// POST /v2/{name}/blobs/uploads - Start upload (unauthenticated, unclaimed project)
#[tokio::test]
async fn test_blob_upload_start_unclaimed() {
    let server = TestServer::new().await;

    // Push to a new project slug (will auto-create unclaimed project)
    let resp = server
        .client
        .post(server.api_url("/v2/unclaimed-blob-project/blobs/uploads"))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    assert!(resp.headers().contains_key("location"));
}

// PUT /v2/{name}/blobs/uploads - Monolithic upload
#[tokio::test]
async fn test_blob_monolithic_upload() {
    let server = TestServer::new().await;
    let user = server.signup("Mono User", "blobmono@example.com").await;
    let org = server.create_org(&user, "Mono Org").await;
    let project = server.create_project(&user, &org, "Mono Project").await;

    let oci_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    let blob_data = b"hello world blob content";
    let digest = compute_digest(blob_data);

    let resp = server
        .client
        .put(server.api_url(&format!(
            "/v2/{}/blobs/uploads?digest={}",
            project_slug, digest
        )))
        .header("Authorization", format!("Bearer {}", oci_token))
        .header("Content-Type", "application/octet-stream")
        .body(blob_data.to_vec())
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
    assert!(resp.headers().contains_key("location"));
    assert!(resp.headers().contains_key("docker-content-digest"));
}

// HEAD /v2/{name}/blobs/{digest} - Check blob exists
#[tokio::test]
async fn test_blob_exists() {
    let server = TestServer::new().await;
    let user = server.signup("Exists User", "blobexists@example.com").await;
    let org = server.create_org(&user, "Exists Org").await;
    let project = server.create_project(&user, &org, "Exists Project").await;

    let oci_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // First upload a blob
    let blob_data = b"blob exists test data";
    let digest = compute_digest(blob_data);

    let upload_resp = server
        .client
        .put(server.api_url(&format!(
            "/v2/{}/blobs/uploads?digest={}",
            project_slug, digest
        )))
        .header("Authorization", format!("Bearer {}", oci_token))
        .header("Content-Type", "application/octet-stream")
        .body(blob_data.to_vec())
        .send()
        .await
        .expect("Upload failed");
    assert_eq!(upload_resp.status(), StatusCode::CREATED);

    // Now check if it exists
    let pull_token = server.oci_pull_token(&user, &project);
    let resp = server
        .client
        .head(server.api_url(&format!("/v2/{}/blobs/{}", project_slug, digest)))
        .header("Authorization", format!("Bearer {}", pull_token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    assert!(resp.headers().contains_key("docker-content-digest"));
    assert!(resp.headers().contains_key("content-length"));
}

// HEAD /v2/{name}/blobs/{digest} - Blob not found
#[tokio::test]
async fn test_blob_not_found() {
    let server = TestServer::new().await;
    let user = server
        .signup("NotFound User", "blobnotfound@example.com")
        .await;
    let org = server.create_org(&user, "NotFound Org").await;
    let project = server.create_project(&user, &org, "NotFound Project").await;

    let oci_token = server.oci_pull_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();
    let fake_digest = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

    let resp = server
        .client
        .head(server.api_url(&format!("/v2/{}/blobs/{}", project_slug, fake_digest)))
        .header("Authorization", format!("Bearer {}", oci_token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// GET /v2/{name}/blobs/{digest} - Download blob
#[tokio::test]
async fn test_blob_get() {
    let server = TestServer::new().await;
    let user = server.signup("Get User", "blobget@example.com").await;
    let org = server.create_org(&user, "Get Org").await;
    let project = server.create_project(&user, &org, "Get Project").await;

    let push_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Upload a blob
    let blob_data = b"blob download test data";
    let digest = compute_digest(blob_data);

    let upload_resp = server
        .client
        .put(server.api_url(&format!(
            "/v2/{}/blobs/uploads?digest={}",
            project_slug, digest
        )))
        .header("Authorization", format!("Bearer {}", push_token))
        .header("Content-Type", "application/octet-stream")
        .body(blob_data.to_vec())
        .send()
        .await
        .expect("Upload failed");
    assert_eq!(upload_resp.status(), StatusCode::CREATED);

    // Download it
    let pull_token = server.oci_pull_token(&user, &project);
    let resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/blobs/{}", project_slug, digest)))
        .header("Authorization", format!("Bearer {}", pull_token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.bytes().await.expect("Failed to read body");
    assert_eq!(body.as_ref(), blob_data);
}

// DELETE /v2/{name}/blobs/{digest} - Delete blob
#[tokio::test]
async fn test_blob_delete() {
    let server = TestServer::new().await;
    let user = server.signup("Delete User", "blobdelete@example.com").await;
    let org = server.create_org(&user, "Delete Org").await;
    let project = server.create_project(&user, &org, "Delete Project").await;

    let push_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Upload a blob
    let blob_data = b"blob delete test data";
    let digest = compute_digest(blob_data);

    let upload_resp = server
        .client
        .put(server.api_url(&format!(
            "/v2/{}/blobs/uploads?digest={}",
            project_slug, digest
        )))
        .header("Authorization", format!("Bearer {}", push_token))
        .header("Content-Type", "application/octet-stream")
        .body(blob_data.to_vec())
        .send()
        .await
        .expect("Upload failed");
    assert_eq!(upload_resp.status(), StatusCode::CREATED);

    // Delete it
    let resp = server
        .client
        .delete(server.api_url(&format!("/v2/{}/blobs/{}", project_slug, digest)))
        .header("Authorization", format!("Bearer {}", push_token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::ACCEPTED);

    // Verify it's gone
    let pull_token = server.oci_pull_token(&user, &project);
    let check_resp = server
        .client
        .head(server.api_url(&format!("/v2/{}/blobs/{}", project_slug, digest)))
        .header("Authorization", format!("Bearer {}", pull_token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(check_resp.status(), StatusCode::NOT_FOUND);
}

// OPTIONS /v2/{name}/blobs/{ref} - CORS preflight
#[tokio::test]
async fn test_blob_options() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .request(
            reqwest::Method::OPTIONS,
            server.api_url("/v2/test-project/blobs/uploads"),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    assert!(resp.headers().contains_key("access-control-allow-origin"));
}

// =============================================================================
// Tests for validate_push_access branches
// =============================================================================

// Unauthenticated push to a CLAIMED project should be rejected with 401
#[tokio::test]
async fn test_blob_upload_unauthenticated_to_claimed_project() {
    let server = TestServer::new().await;

    // Create a user, org, and project (this makes it "claimed" since user is a member)
    let user = server
        .signup("Claimed User", "claimedproject@example.com")
        .await;
    let org = server.create_org(&user, "Claimed Org").await;
    let project = server.create_project(&user, &org, "Claimed Project").await;

    let project_slug: &str = project.slug.as_ref();

    // Try to push without authentication - should fail with 401
    let resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", project_slug)))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert!(resp.headers().contains_key("www-authenticate"));
}

// Authenticated push to an UNCLAIMED project should auto-claim the org
#[tokio::test]
async fn test_blob_upload_authenticated_to_unclaimed_project() {
    let server = TestServer::new().await;

    // First, create an unclaimed project by pushing without authentication
    let unclaimed_slug = "unclaimed-to-claim-project";

    let create_resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", unclaimed_slug)))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(create_resp.status(), StatusCode::ACCEPTED);

    // Now create a user and push with authentication - should auto-claim
    let user = server.signup("Claimer User", "claimer@example.com").await;

    let oci_token = server.oci_token(&user, unclaimed_slug, &[OciAction::Push]);

    let resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", unclaimed_slug)))
        .header("Authorization", format!("Bearer {}", oci_token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::ACCEPTED);

    // Verify the project is now claimed by trying unauthenticated push again
    // It should now be rejected since the org was claimed
    let unauth_resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", unclaimed_slug)))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(unauth_resp.status(), StatusCode::UNAUTHORIZED);
}

// A structurally valid but wrongly-signed JWT should NOT silently downgrade
// to public access. Even on an unclaimed project, a bad signature must be rejected.
#[tokio::test]
async fn test_blob_upload_invalid_token_no_downgrade() {
    let server = TestServer::new().await;

    // First, create an unclaimed project (unauthenticated push succeeds)
    let unclaimed_slug = "auth-downgrade-test-project";
    let create_resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", unclaimed_slug)))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(create_resp.status(), StatusCode::ACCEPTED);

    // Get a valid token, then tamper with it to create a structurally valid
    // but wrongly-signed JWT (flip a char in the signature)
    let user = server
        .signup("Downgrade User", "downgrade@example.com")
        .await;
    let valid_token = server.oci_token(&user, unclaimed_slug, &[OciAction::Push]);
    // Corrupt the last character of the signature portion
    let mut tampered = valid_token.clone();
    let last = tampered.pop().unwrap_or('A');
    tampered.push(if last == 'A' { 'B' } else { 'A' });

    let resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", unclaimed_slug)))
        .header("Authorization", format!("Bearer {}", tampered))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "Tampered JWT must be rejected, not silently downgraded to public access"
    );
}

// Push to a non-existent project by UUID should return 404
#[tokio::test]
async fn test_blob_upload_nonexistent_uuid() {
    let server = TestServer::new().await;
    let user = server.signup("UUID User", "uuidpush@example.com").await;

    // Use a random UUID that doesn't exist
    let fake_uuid = "00000000-0000-0000-0000-000000000000";

    let oci_token = server.oci_token(&user, fake_uuid, &[OciAction::Push]);

    let resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", fake_uuid)))
        .header("Authorization", format!("Bearer {}", oci_token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// Push to a non-existent project by slug should auto-create the project
#[tokio::test]
async fn test_blob_upload_nonexistent_slug_authenticated() {
    let server = TestServer::new().await;
    let user = server.signup("Slug User", "slugpush@example.com").await;

    // Use a new slug that doesn't exist yet
    let new_slug = "auto-created-project";

    let oci_token = server.oci_token(&user, new_slug, &[OciAction::Push]);

    let resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", new_slug)))
        .header("Authorization", format!("Bearer {}", oci_token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    assert!(resp.headers().contains_key("location"));

    // The project should now exist and be claimed by the user
    // Verify by trying unauthenticated push - should be rejected
    let unauth_resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", new_slug)))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(unauth_resp.status(), StatusCode::UNAUTHORIZED);
}

// Monolithic upload (PUT /blobs/uploads?digest=) to unclaimed project
#[tokio::test]
async fn test_blob_monolithic_upload_unclaimed() {
    let server = TestServer::new().await;

    let blob_data = b"unclaimed monolithic blob content";
    let digest = compute_digest(blob_data);

    // Push to a new project slug (will auto-create unclaimed project)
    let resp = server
        .client
        .put(server.api_url(&format!(
            "/v2/unclaimed-monolithic-project/blobs/uploads?digest={}",
            digest
        )))
        .header("Content-Type", "application/octet-stream")
        .body(blob_data.to_vec())
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
    assert!(resp.headers().contains_key("location"));
    assert!(resp.headers().contains_key("docker-content-digest"));
}

// Monolithic upload to a CLAIMED project without auth should fail
#[tokio::test]
async fn test_blob_monolithic_upload_unauthenticated_to_claimed() {
    let server = TestServer::new().await;

    // Create a claimed project
    let user = server
        .signup("Mono Claimed User", "monoclaimedproject@example.com")
        .await;
    let org = server.create_org(&user, "Mono Claimed Org").await;
    let project = server
        .create_project(&user, &org, "Mono Claimed Project")
        .await;

    let project_slug: &str = project.slug.as_ref();

    let blob_data = b"claimed monolithic blob content";
    let digest = compute_digest(blob_data);

    // Try to push without authentication - should fail
    let resp = server
        .client
        .put(server.api_url(&format!(
            "/v2/{}/blobs/uploads?digest={}",
            project_slug, digest
        )))
        .header("Content-Type", "application/octet-stream")
        .body(blob_data.to_vec())
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}
