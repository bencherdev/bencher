#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args,
    clippy::similar_names,
    clippy::indexing_slicing,
    clippy::integer_division
)]
//! Integration tests for OCI blob endpoints.

use bencher_api_tests::TestServer;
use bencher_api_tests::oci::compute_digest;
use bencher_json::RunnerUuid;
use bencher_token::OciAction;
use http::StatusCode;

// POST /v2/{name}/blobs/uploads - Start upload (authenticated)
#[tokio::test]
async fn blob_upload_start() {
    let server = TestServer::new().await;
    let user = server.signup("Blob User", "blobstart@example.com").await;
    let org = server.create_org(&user, "Blob Org").await;
    let project = server.create_project(&user, &org, "Blob Project").await;

    let oci_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    let resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&oci_token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    assert!(resp.headers().contains_key("location"));
    assert!(resp.headers().contains_key("docker-upload-uuid"));
}

// POST /v2/{name}/blobs/uploads - Start upload (unauthenticated, unclaimed project)
#[tokio::test]
async fn blob_upload_start_unclaimed() {
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
async fn blob_monolithic_upload() {
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&oci_token),
        )
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
async fn blob_exists() {
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&oci_token),
        )
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&pull_token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    assert!(resp.headers().contains_key("docker-content-digest"));
    assert!(resp.headers().contains_key("content-length"));
    assert!(!resp.headers().contains_key("accept-ranges"));
}

// HEAD /v2/{name}/blobs/{digest} - Blob not found
#[tokio::test]
async fn blob_not_found() {
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&oci_token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// GET /v2/{name}/blobs/{digest} - Download blob
#[tokio::test]
async fn blob_get() {
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&push_token),
        )
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&pull_token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    assert!(!resp.headers().contains_key("accept-ranges"));
    let body = resp.bytes().await.expect("Failed to read body");
    assert_eq!(body.as_ref(), blob_data);
}

// DELETE /v2/{name}/blobs/{digest} - Delete blob
#[tokio::test]
async fn blob_delete() {
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&push_token),
        )
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&push_token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::ACCEPTED);

    // Verify it's gone
    let pull_token = server.oci_pull_token(&user, &project);
    let check_resp = server
        .client
        .head(server.api_url(&format!("/v2/{}/blobs/{}", project_slug, digest)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&pull_token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(check_resp.status(), StatusCode::NOT_FOUND);
}

// OPTIONS /v2/{name}/blobs/{ref} - CORS preflight
#[tokio::test]
async fn blob_options() {
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
async fn blob_upload_unauthenticated_to_claimed_project() {
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
async fn blob_upload_authenticated_to_unclaimed_project() {
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
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&oci_token),
        )
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
async fn blob_upload_invalid_token_no_downgrade() {
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
    // but wrongly-signed JWT (flip a char in the middle of the signature).
    // We must preserve valid JWT structure (3 dot-separated base64url parts)
    // so that Jwt::parse() succeeds; otherwise the token is silently treated
    // as absent and the unclaimed project allows unauthenticated push.
    let user = server
        .signup("Downgrade User", "downgrade@example.com")
        .await;
    let valid_token = server.oci_token(&user, unclaimed_slug, &[OciAction::Push]);
    // Split into header.payload.signature and flip a char in the signature middle
    let parts: Vec<&str> = valid_token.split('.').collect();
    assert_eq!(parts.len(), 3, "JWT should have 3 parts");
    let sig = parts[2];
    let mid = sig.len() / 2;
    let mut sig_bytes: Vec<u8> = sig.bytes().collect();
    // Flip between two valid base64url characters
    sig_bytes[mid] = if sig_bytes[mid] == b'A' { b'B' } else { b'A' };
    let tampered_sig = String::from_utf8(sig_bytes).expect("valid UTF-8");
    let tampered = format!("{}.{}.{}", parts[0], parts[1], tampered_sig);

    let resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", unclaimed_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&tampered),
        )
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
async fn blob_upload_nonexistent_uuid() {
    let server = TestServer::new().await;
    let user = server.signup("UUID User", "uuidpush@example.com").await;

    // Use a random UUID that doesn't exist
    let fake_uuid = "00000000-0000-0000-0000-000000000000";

    let oci_token = server.oci_token(&user, fake_uuid, &[OciAction::Push]);

    let resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", fake_uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&oci_token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// Push to a non-existent project by slug should auto-create the project
#[tokio::test]
async fn blob_upload_nonexistent_slug_authenticated() {
    let server = TestServer::new().await;
    let user = server.signup("Slug User", "slugpush@example.com").await;

    // Use a new slug that doesn't exist yet
    let new_slug = "auto-created-project";

    let oci_token = server.oci_token(&user, new_slug, &[OciAction::Push]);

    let resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", new_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&oci_token),
        )
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
async fn blob_monolithic_upload_unclaimed() {
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
async fn blob_monolithic_upload_unauthenticated_to_claimed() {
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

// PUT /v2/{name}/blobs/uploads?digest= - Monolithic upload with wrong digest
#[tokio::test]
async fn blob_monolithic_upload_digest_mismatch() {
    let server = TestServer::new().await;
    let user = server
        .signup("DigestMismatch User", "blobdigestmismatch@example.com")
        .await;
    let org = server.create_org(&user, "DigestMismatch Org").await;
    let project = server
        .create_project(&user, &org, "DigestMismatch Project")
        .await;

    let oci_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    let blob_data = b"actual blob content";
    let wrong_digest = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

    let resp = server
        .client
        .put(server.api_url(&format!(
            "/v2/{}/blobs/uploads?digest={}",
            project_slug, wrong_digest
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&oci_token),
        )
        .header("Content-Type", "application/octet-stream")
        .body(blob_data.to_vec())
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "Digest mismatch should be rejected"
    );
}

// PUT /v2/{name}/blobs/uploads?digest= - Monolithic upload with zero-length body
#[tokio::test]
async fn blob_monolithic_upload_zero_length() {
    let server = TestServer::new().await;
    let user = server
        .signup("ZeroLen User", "blobzerolen@example.com")
        .await;
    let org = server.create_org(&user, "ZeroLen Org").await;
    let project = server.create_project(&user, &org, "ZeroLen Project").await;

    let oci_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    let blob_data = b"";
    let digest = compute_digest(blob_data);

    let resp = server
        .client
        .put(server.api_url(&format!(
            "/v2/{}/blobs/uploads?digest={}",
            project_slug, digest
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&oci_token),
        )
        .header("Content-Type", "application/octet-stream")
        .body(blob_data.to_vec())
        .send()
        .await
        .expect("Request failed");

    // Zero-length blob upload is rejected by the storage layer
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "Zero-length monolithic upload should be rejected"
    );
}

// =============================================================================
// Cross-Repo Mount Tests
// =============================================================================

// Push blob to project A, attempt mount to project B with a token scoped only to B
// Since user B doesn't have pull access to project A, the mount should fail silently
// and fall through to a regular upload (returning 202 Accepted, not 201 Created)
#[tokio::test]
async fn cross_repo_mount_access_denied_fallback() {
    let server = TestServer::new().await;

    // Create user A with project A and push a blob
    let user_a = server.signup("MountUserA", "mountusera@example.com").await;
    let org_a = server.create_org(&user_a, "MountOrgA").await;
    let project_a = server
        .create_project(&user_a, &org_a, "MountProjectA")
        .await;

    let push_token_a = server.oci_push_token(&user_a, &project_a);
    let project_a_slug: &str = project_a.slug.as_ref();

    let blob_data = b"cross-repo mount test blob";
    let digest = compute_digest(blob_data);

    // Upload blob to project A
    let upload_resp = server
        .client
        .put(server.api_url(&format!(
            "/v2/{}/blobs/uploads?digest={}",
            project_a_slug, digest
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&push_token_a),
        )
        .header("Content-Type", "application/octet-stream")
        .body(blob_data.to_vec())
        .send()
        .await
        .expect("Upload to project A failed");
    assert_eq!(upload_resp.status(), StatusCode::CREATED);

    // Create user B with project B
    let user_b = server.signup("MountUserB", "mountuserb@example.com").await;
    let org_b = server.create_org(&user_b, "MountOrgB").await;
    let project_b = server
        .create_project(&user_b, &org_b, "MountProjectB")
        .await;

    // User B's token only has push access to project B
    let push_token_b = server.oci_push_token(&user_b, &project_b);
    let project_b_slug: &str = project_b.slug.as_ref();

    // Attempt cross-repo mount: from project A to project B using user B's token
    // User B does NOT have pull access to project A, so mount should fail silently
    let mount_resp = server
        .client
        .post(server.api_url(&format!(
            "/v2/{}/blobs/uploads?digest={}&from={}",
            project_b_slug, digest, project_a_slug
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&push_token_b),
        )
        .send()
        .await
        .expect("Mount request failed");

    // Should get 202 Accepted (fallback to regular upload), NOT 201 Created (successful mount)
    assert_eq!(
        mount_resp.status(),
        StatusCode::ACCEPTED,
        "Cross-repo mount without pull access to source should fall back to regular upload (202)"
    );
    assert!(
        mount_resp.headers().contains_key("location"),
        "Fallback response should include upload location"
    );
    assert!(
        mount_resp.headers().contains_key("docker-upload-uuid"),
        "Fallback response should include upload UUID"
    );
}

// =============================================================================
// SHA-512 Digest Tests
// =============================================================================

// Upload blob with sha512 digest should fail because storage only computes sha256
#[tokio::test]
async fn blob_upload_sha512() {
    let server = TestServer::new().await;
    let user = server.signup("Sha512 User", "sha512blob@example.com").await;
    let org = server.create_org(&user, "Sha512 Org").await;
    let project = server.create_project(&user, &org, "Sha512 Project").await;

    let oci_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    let blob_data = b"sha512 digest test data";
    // Use a valid sha512 digest (128 hex chars) - it won't match the actual content
    // because the server computes sha256 internally
    let sha512_digest = "sha512:cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e";

    let resp = server
        .client
        .put(server.api_url(&format!(
            "/v2/{}/blobs/uploads?digest={}",
            project_slug, sha512_digest
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&oci_token),
        )
        .header("Content-Type", "application/octet-stream")
        .body(blob_data.to_vec())
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "SHA-512 digest should be rejected since storage only supports SHA-256"
    );
}

// =============================================================================
// Authorization Scope Enforcement Tests
// =============================================================================

// Pull-only token should NOT be able to start an upload (push operation)
#[tokio::test]
async fn blob_upload_pull_only_token_rejected() {
    let server = TestServer::new().await;
    let user = server
        .signup("PullOnly User", "pullonlyblob@example.com")
        .await;
    let org = server.create_org(&user, "PullOnly Org").await;
    let project = server.create_project(&user, &org, "PullOnly Project").await;

    // Get a pull-only token
    let pull_token = server.oci_pull_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Attempt to start an upload (push operation) with pull-only token
    let resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&pull_token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "Pull-only token should not be able to start an upload"
    );
}

// =============================================================================
// Max Body Size Tests
// =============================================================================

// Monolithic blob upload exceeding max body size should be rejected
#[tokio::test]
async fn blob_monolithic_upload_exceeds_max_body_size() {
    // max_body_size = 100 bytes
    let server = TestServer::new_with_limits(3600, 100).await;
    let user = server
        .signup("MonoLimit User", "monolimit@example.com")
        .await;
    let org = server.create_org(&user, "MonoLimit Org").await;
    let project = server
        .create_project(&user, &org, "MonoLimit Project")
        .await;

    let oci_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // 200-byte blob exceeds 100-byte max_body_size
    let blob_data = vec![0xCC; 200];
    let digest = compute_digest(&blob_data);

    let resp = server
        .client
        .put(server.api_url(&format!(
            "/v2/{}/blobs/uploads?digest={}",
            project_slug, digest
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&oci_token),
        )
        .header("Content-Type", "application/octet-stream")
        .body(blob_data)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        resp.status(),
        StatusCode::PAYLOAD_TOO_LARGE,
        "Monolithic blob exceeding max body size should be rejected, got {}",
        resp.status()
    );
}

// =============================================================================
// Storage Failure Injection Tests
// =============================================================================

// Blob read after storage directory is deleted
#[tokio::test]
async fn blob_read_after_storage_deleted() {
    let server = TestServer::new().await;
    let user = server
        .signup("StorageFail User", "storagefailblob@example.com")
        .await;
    let org = server.create_org(&user, "StorageFail Org").await;
    let project = server
        .create_project(&user, &org, "StorageFail Project")
        .await;

    let push_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Upload a blob successfully
    let blob_data = b"blob for storage failure test";
    let digest = compute_digest(blob_data);

    let upload_resp = server
        .client
        .put(server.api_url(&format!(
            "/v2/{}/blobs/uploads?digest={}",
            project_slug, digest
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&push_token),
        )
        .header("Content-Type", "application/octet-stream")
        .body(blob_data.to_vec())
        .send()
        .await
        .expect("Upload failed");
    assert_eq!(upload_resp.status(), StatusCode::CREATED);

    // Delete project's OCI storage directory
    let project_oci_dir = server
        .db_path()
        .parent()
        .unwrap()
        .join("oci")
        .join(project.uuid.to_string());
    tokio::fs::remove_dir_all(&project_oci_dir)
        .await
        .expect("Failed to delete project OCI storage directory");

    // Try to GET the blob - should fail with 404
    let pull_token = server.oci_pull_token(&user, &project);
    let resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/blobs/{}", project_slug, digest)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&pull_token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "Blob read after storage deleted should return 404"
    );
}

// Push-only token should NOT be able to read a blob (pull operation)
#[tokio::test]
async fn blob_head_push_only_token_rejected() {
    let server = TestServer::new().await;
    let user = server
        .signup("PushOnly User", "pushonlyblob@example.com")
        .await;
    let org = server.create_org(&user, "PushOnly Org").await;
    let project = server.create_project(&user, &org, "PushOnly Project").await;

    let push_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Upload a blob first
    let blob_data = b"push-only token test data";
    let digest = compute_digest(blob_data);

    let upload_resp = server
        .client
        .put(server.api_url(&format!(
            "/v2/{}/blobs/uploads?digest={}",
            project_slug, digest
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&push_token),
        )
        .header("Content-Type", "application/octet-stream")
        .body(blob_data.to_vec())
        .send()
        .await
        .expect("Upload failed");
    assert_eq!(upload_resp.status(), StatusCode::CREATED);

    // Attempt HEAD (pull operation) with push-only token
    let resp = server
        .client
        .head(server.api_url(&format!("/v2/{}/blobs/{}", project_slug, digest)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&push_token),
        )
        .send()
        .await
        .expect("Request failed");

    // require_pull_access intentionally maps all auth errors (including wrong action)
    // to 401 with WWW-Authenticate to avoid leaking whether the token was invalid
    // or just lacked the correct scope
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "Push-only token should not be able to read a blob"
    );
}

// =============================================================================
// Runner OCI Token Pull Tests
// =============================================================================

// Runner OCI token should be able to pull (GET) a blob
#[tokio::test]
async fn blob_get_runner_oci_token() {
    let server = TestServer::new().await;
    let user = server
        .signup("RunnerPull User", "runnerpullblob@example.com")
        .await;
    let org = server.create_org(&user, "RunnerPull Org").await;
    let project = server
        .create_project(&user, &org, "RunnerPull Project")
        .await;

    let push_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Upload a blob with a user push token
    let blob_data = b"runner pull test blob content";
    let digest = compute_digest(blob_data);

    let upload_resp = server
        .client
        .put(server.api_url(&format!(
            "/v2/{}/blobs/uploads?digest={}",
            project_slug, digest
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&push_token),
        )
        .header("Content-Type", "application/octet-stream")
        .body(blob_data.to_vec())
        .send()
        .await
        .expect("Upload failed");
    assert_eq!(upload_resp.status(), StatusCode::CREATED);

    // Pull using a runner OCI token
    let runner_uuid: RunnerUuid = "00000000-0000-4000-8000-000000000001"
        .parse()
        .expect("valid UUID");
    let runner_token = server.oci_runner_pull_token(runner_uuid, project_slug);

    let resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/blobs/{}", project_slug, digest)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&runner_token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "Runner OCI token should be able to pull a blob"
    );
    let body = resp.bytes().await.expect("Failed to read body");
    assert_eq!(body.as_ref(), blob_data);
}

// Runner OCI token should be able to check (HEAD) a blob exists
#[tokio::test]
async fn blob_head_runner_oci_token() {
    let server = TestServer::new().await;
    let user = server
        .signup("RunnerHead User", "runnerheadblob@example.com")
        .await;
    let org = server.create_org(&user, "RunnerHead Org").await;
    let project = server
        .create_project(&user, &org, "RunnerHead Project")
        .await;

    let push_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Upload a blob
    let blob_data = b"runner head test blob content";
    let digest = compute_digest(blob_data);

    let upload_resp = server
        .client
        .put(server.api_url(&format!(
            "/v2/{}/blobs/uploads?digest={}",
            project_slug, digest
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&push_token),
        )
        .header("Content-Type", "application/octet-stream")
        .body(blob_data.to_vec())
        .send()
        .await
        .expect("Upload failed");
    assert_eq!(upload_resp.status(), StatusCode::CREATED);

    // HEAD using a runner OCI token
    let runner_uuid: RunnerUuid = "00000000-0000-4000-8000-000000000002"
        .parse()
        .expect("valid UUID");
    let runner_token = server.oci_runner_pull_token(runner_uuid, project_slug);

    let resp = server
        .client
        .head(server.api_url(&format!("/v2/{}/blobs/{}", project_slug, digest)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&runner_token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "Runner OCI token should be able to HEAD a blob"
    );
    assert!(resp.headers().contains_key("docker-content-digest"));
}

// Runner OCI token scoped to a different repository should be rejected
#[tokio::test]
async fn blob_get_runner_oci_token_wrong_repo() {
    let server = TestServer::new().await;
    let user = server
        .signup("RunnerWrongRepo User", "runnerwrongrepo@example.com")
        .await;
    let org = server.create_org(&user, "RunnerWrongRepo Org").await;
    let project = server
        .create_project(&user, &org, "RunnerWrongRepo Project")
        .await;

    let push_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Upload a blob
    let blob_data = b"runner wrong repo test blob";
    let digest = compute_digest(blob_data);

    let upload_resp = server
        .client
        .put(server.api_url(&format!(
            "/v2/{}/blobs/uploads?digest={}",
            project_slug, digest
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&push_token),
        )
        .header("Content-Type", "application/octet-stream")
        .body(blob_data.to_vec())
        .send()
        .await
        .expect("Upload failed");
    assert_eq!(upload_resp.status(), StatusCode::CREATED);

    // Try to pull with a runner token scoped to a DIFFERENT repository
    let runner_uuid: RunnerUuid = "00000000-0000-4000-8000-000000000003"
        .parse()
        .expect("valid UUID");
    let wrong_repo_token = server.oci_runner_pull_token(runner_uuid, "some-other-project");

    let resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/blobs/{}", project_slug, digest)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&wrong_repo_token),
        )
        .send()
        .await
        .expect("Request failed");

    // require_pull_access maps all auth/scope errors to 401 with WWW-Authenticate
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "Runner token scoped to wrong repository should be rejected"
    );
}

// Runner OCI token with only Push action should be rejected for pull
#[tokio::test]
async fn blob_get_runner_oci_token_push_only() {
    let server = TestServer::new().await;
    let user = server
        .signup("RunnerPushOnly User", "runnerpushonly@example.com")
        .await;
    let org = server.create_org(&user, "RunnerPushOnly Org").await;
    let project = server
        .create_project(&user, &org, "RunnerPushOnly Project")
        .await;

    let push_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Upload a blob
    let blob_data = b"runner push-only action test blob";
    let digest = compute_digest(blob_data);

    let upload_resp = server
        .client
        .put(server.api_url(&format!(
            "/v2/{}/blobs/uploads?digest={}",
            project_slug, digest
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&push_token),
        )
        .header("Content-Type", "application/octet-stream")
        .body(blob_data.to_vec())
        .send()
        .await
        .expect("Upload failed");
    assert_eq!(upload_resp.status(), StatusCode::CREATED);

    // Try to pull with a runner token that only has Push action (not Pull)
    let runner_uuid: RunnerUuid = "00000000-0000-4000-8000-000000000004"
        .parse()
        .expect("valid UUID");
    let push_only_runner_token =
        server.oci_runner_token(runner_uuid, project_slug, &[OciAction::Push]);

    let resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/blobs/{}", project_slug, digest)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&push_only_runner_token),
        )
        .send()
        .await
        .expect("Request failed");

    // require_pull_access maps all auth/scope errors to 401 with WWW-Authenticate
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "Runner token with only Push action should be rejected for pull"
    );
}
