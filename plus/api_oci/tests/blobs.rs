#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args,
    clippy::redundant_test_prefix
)]
//! Integration tests for OCI blob endpoints.

use bencher_api_tests::TestServer;
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
