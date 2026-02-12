#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args,
    clippy::too_many_lines,
    clippy::decimal_literal_representation
)]
//! Integration tests for OCI upload session endpoints.

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use bencher_api_tests::TestServer;
use bencher_api_tests::oci::compute_digest;
use http::StatusCode;

/// Helper to extract session ID from Location header
fn extract_session_id(location: &str) -> Option<String> {
    // Location format: /v2/{name}/blobs/uploads/{session_id}
    location.split('/').next_back().map(ToOwned::to_owned)
}

// GET /v2/{name}/blobs/uploads/{session_id} - Get upload status
#[tokio::test]
async fn upload_status() {
    let server = TestServer::new().await;
    let user = server
        .signup("Status User", "uploadstatus@example.com")
        .await;
    let org = server.create_org(&user, "Status Org").await;
    let project = server.create_project(&user, &org, "Status Project").await;

    let oci_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Start an upload
    let start_resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", project_slug)))
        .header("Authorization", format!("Bearer {}", oci_token))
        .send()
        .await
        .expect("Start upload failed");

    assert_eq!(start_resp.status(), StatusCode::ACCEPTED);
    let location = start_resp
        .headers()
        .get("location")
        .expect("Missing location header")
        .to_str()
        .expect("Invalid location header");
    let session_id = extract_session_id(location).expect("Invalid location format");

    // Check upload status (no auth needed - session ID is the auth)
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}",
            project_slug, session_id
        )))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    assert!(resp.headers().contains_key("range"));
    assert!(resp.headers().contains_key("docker-upload-uuid"));
}

// PATCH /v2/{name}/blobs/uploads/{session_id} - Upload chunk
#[tokio::test]
async fn upload_chunk() {
    let server = TestServer::new().await;
    let user = server.signup("Chunk User", "uploadchunk@example.com").await;
    let org = server.create_org(&user, "Chunk Org").await;
    let project = server.create_project(&user, &org, "Chunk Project").await;

    let oci_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Start an upload
    let start_resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", project_slug)))
        .header("Authorization", format!("Bearer {}", oci_token))
        .send()
        .await
        .expect("Start upload failed");

    let location = start_resp
        .headers()
        .get("location")
        .expect("Missing location header")
        .to_str()
        .expect("Invalid location header");
    let session_id = extract_session_id(location).expect("Invalid location format");

    // Upload a chunk
    let chunk_data = b"first chunk of data";
    let resp = server
        .client
        .patch(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}",
            project_slug, session_id
        )))
        .header("Content-Type", "application/octet-stream")
        .body(chunk_data.to_vec())
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    assert!(resp.headers().contains_key("range"));

    // Check the range reflects the uploaded data
    let range = resp
        .headers()
        .get("range")
        .expect("Missing range header")
        .to_str()
        .expect("Invalid range header");
    assert!(range.contains(&format!("0-{}", chunk_data.len() - 1)));
}

// PATCH with Content-Range header
#[tokio::test]
async fn upload_chunk_with_content_range() {
    let server = TestServer::new().await;
    let user = server.signup("Range User", "uploadrange@example.com").await;
    let org = server.create_org(&user, "Range Org").await;
    let project = server.create_project(&user, &org, "Range Project").await;

    let oci_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Start an upload
    let start_resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", project_slug)))
        .header("Authorization", format!("Bearer {}", oci_token))
        .send()
        .await
        .expect("Start upload failed");

    let location = start_resp
        .headers()
        .get("location")
        .expect("Missing location header")
        .to_str()
        .expect("Invalid location header");
    let session_id = extract_session_id(location).expect("Invalid location format");

    // Upload first chunk
    let chunk1 = b"first chunk";
    let resp1 = server
        .client
        .patch(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}",
            project_slug, session_id
        )))
        .header("Content-Type", "application/octet-stream")
        .header("Content-Range", format!("0-{}", chunk1.len() - 1))
        .body(chunk1.to_vec())
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp1.status(), StatusCode::ACCEPTED);

    // Upload second chunk with correct range
    let chunk2 = b"second chunk";
    let start = chunk1.len();
    let end = start + chunk2.len() - 1;
    let resp2 = server
        .client
        .patch(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}",
            project_slug, session_id
        )))
        .header("Content-Type", "application/octet-stream")
        .header("Content-Range", format!("{}-{}", start, end))
        .body(chunk2.to_vec())
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp2.status(), StatusCode::ACCEPTED);
}

// PATCH with wrong Content-Range (should return 416)
#[tokio::test]
async fn upload_chunk_wrong_range() {
    let server = TestServer::new().await;
    let user = server
        .signup("WrongRange User", "uploadwrongrange@example.com")
        .await;
    let org = server.create_org(&user, "WrongRange Org").await;
    let project = server
        .create_project(&user, &org, "WrongRange Project")
        .await;

    let oci_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Start an upload
    let start_resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", project_slug)))
        .header("Authorization", format!("Bearer {}", oci_token))
        .send()
        .await
        .expect("Start upload failed");

    let location = start_resp
        .headers()
        .get("location")
        .expect("Missing location header")
        .to_str()
        .expect("Invalid location header");
    let session_id = extract_session_id(location).expect("Invalid location format");

    // Try to upload with wrong range (starting at 100 when nothing uploaded yet)
    let chunk = b"some data";
    let resp = server
        .client
        .patch(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}",
            project_slug, session_id
        )))
        .header("Content-Type", "application/octet-stream")
        .header("Content-Range", "100-108")
        .body(chunk.to_vec())
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::RANGE_NOT_SATISFIABLE);
}

// PUT /v2/{name}/blobs/uploads/{session_id}?digest= - Complete upload
#[tokio::test]
async fn upload_complete() {
    let server = TestServer::new().await;
    let user = server
        .signup("Complete User", "uploadcomplete@example.com")
        .await;
    let org = server.create_org(&user, "Complete Org").await;
    let project = server.create_project(&user, &org, "Complete Project").await;

    let oci_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Start an upload
    let start_resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", project_slug)))
        .header("Authorization", format!("Bearer {}", oci_token))
        .send()
        .await
        .expect("Start upload failed");

    let location = start_resp
        .headers()
        .get("location")
        .expect("Missing location header")
        .to_str()
        .expect("Invalid location header");
    let session_id = extract_session_id(location).expect("Invalid location format");

    // Upload chunk
    let chunk = b"complete upload test data";
    let _patch_resp = server
        .client
        .patch(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}",
            project_slug, session_id
        )))
        .header("Content-Type", "application/octet-stream")
        .body(chunk.to_vec())
        .send()
        .await
        .expect("Patch failed");

    // Complete the upload
    let digest = compute_digest(chunk);
    let resp = server
        .client
        .put(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}?digest={}",
            project_slug, session_id, digest
        )))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
    assert!(resp.headers().contains_key("location"));
    assert!(resp.headers().contains_key("docker-content-digest"));
}

// PUT with additional data in body
#[tokio::test]
async fn upload_complete_with_final_chunk() {
    let server = TestServer::new().await;
    let user = server
        .signup("FinalChunk User", "uploadfinal@example.com")
        .await;
    let org = server.create_org(&user, "FinalChunk Org").await;
    let project = server
        .create_project(&user, &org, "FinalChunk Project")
        .await;

    let oci_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Start an upload
    let start_resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", project_slug)))
        .header("Authorization", format!("Bearer {}", oci_token))
        .send()
        .await
        .expect("Start upload failed");

    let location = start_resp
        .headers()
        .get("location")
        .expect("Missing location header")
        .to_str()
        .expect("Invalid location header");
    let session_id = extract_session_id(location).expect("Invalid location format");

    // Upload first chunk
    let chunk1 = b"first part ";
    let _patch_resp = server
        .client
        .patch(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}",
            project_slug, session_id
        )))
        .header("Content-Type", "application/octet-stream")
        .body(chunk1.to_vec())
        .send()
        .await
        .expect("Patch failed");

    // Complete with final chunk in body
    let chunk2 = b"second part";
    let mut full_data = Vec::new();
    full_data.extend_from_slice(chunk1);
    full_data.extend_from_slice(chunk2);
    let digest = compute_digest(&full_data);

    let resp = server
        .client
        .put(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}?digest={}",
            project_slug, session_id, digest
        )))
        .header("Content-Type", "application/octet-stream")
        .body(chunk2.to_vec())
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
}

// PUT with wrong digest (should fail)
#[tokio::test]
async fn upload_complete_wrong_digest() {
    let server = TestServer::new().await;
    let user = server
        .signup("WrongDigest User", "uploadwrongdigest@example.com")
        .await;
    let org = server.create_org(&user, "WrongDigest Org").await;
    let project = server
        .create_project(&user, &org, "WrongDigest Project")
        .await;

    let oci_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Start an upload
    let start_resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", project_slug)))
        .header("Authorization", format!("Bearer {}", oci_token))
        .send()
        .await
        .expect("Start upload failed");

    let location = start_resp
        .headers()
        .get("location")
        .expect("Missing location header")
        .to_str()
        .expect("Invalid location header");
    let session_id = extract_session_id(location).expect("Invalid location format");

    // Upload chunk
    let chunk = b"wrong digest test data";
    let _patch_resp = server
        .client
        .patch(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}",
            project_slug, session_id
        )))
        .header("Content-Type", "application/octet-stream")
        .body(chunk.to_vec())
        .send()
        .await
        .expect("Patch failed");

    // Complete with wrong digest
    let wrong_digest = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
    let resp = server
        .client
        .put(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}?digest={}",
            project_slug, session_id, wrong_digest
        )))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// DELETE /v2/{name}/blobs/uploads/{session_id} - Cancel upload
#[tokio::test]
async fn upload_cancel() {
    let server = TestServer::new().await;
    let user = server
        .signup("Cancel User", "uploadcancel@example.com")
        .await;
    let org = server.create_org(&user, "Cancel Org").await;
    let project = server.create_project(&user, &org, "Cancel Project").await;

    let oci_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Start an upload
    let start_resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", project_slug)))
        .header("Authorization", format!("Bearer {}", oci_token))
        .send()
        .await
        .expect("Start upload failed");

    let location = start_resp
        .headers()
        .get("location")
        .expect("Missing location header")
        .to_str()
        .expect("Invalid location header");
    let session_id = extract_session_id(location).expect("Invalid location format");

    // Cancel the upload
    let resp = server
        .client
        .delete(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}",
            project_slug, session_id
        )))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::ACCEPTED);

    // Verify it's cancelled (status should fail)
    let check_resp = server
        .client
        .get(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}",
            project_slug, session_id
        )))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(check_resp.status(), StatusCode::NOT_FOUND);
}

// Upload to a cancelled session should return 404
#[tokio::test]
async fn upload_to_cancelled_session() {
    let server = TestServer::new().await;
    let user = server
        .signup("Cancelled User", "uploadcancelled@example.com")
        .await;
    let org = server.create_org(&user, "Cancelled Org").await;
    let project = server
        .create_project(&user, &org, "Cancelled Project")
        .await;

    let oci_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Start an upload
    let start_resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", project_slug)))
        .header("Authorization", format!("Bearer {}", oci_token))
        .send()
        .await
        .expect("Start upload failed");

    let location = start_resp
        .headers()
        .get("location")
        .expect("Missing location header")
        .to_str()
        .expect("Invalid location header");
    let session_id = extract_session_id(location).expect("Invalid location format");

    // Cancel the upload
    let cancel_resp = server
        .client
        .delete(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}",
            project_slug, session_id
        )))
        .send()
        .await
        .expect("Cancel failed");
    assert_eq!(cancel_resp.status(), StatusCode::ACCEPTED);

    // Try to upload a chunk to the cancelled session
    let chunk = b"data after cancel";
    let resp = server
        .client
        .patch(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}",
            project_slug, session_id
        )))
        .header("Content-Type", "application/octet-stream")
        .body(chunk.to_vec())
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "Upload to cancelled session should return 404"
    );
}

// PATCH with malformed Content-Range (not parseable)
#[tokio::test]
async fn upload_malformed_content_range() {
    let server = TestServer::new().await;
    let user = server
        .signup("MalformedRange User", "uploadmalformedrange@example.com")
        .await;
    let org = server.create_org(&user, "MalformedRange Org").await;
    let project = server
        .create_project(&user, &org, "MalformedRange Project")
        .await;

    let oci_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Start an upload
    let start_resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", project_slug)))
        .header("Authorization", format!("Bearer {}", oci_token))
        .send()
        .await
        .expect("Start upload failed");

    let location = start_resp
        .headers()
        .get("location")
        .expect("Missing location header")
        .to_str()
        .expect("Invalid location header");
    let session_id = extract_session_id(location).expect("Invalid location format");

    // Upload with a completely unparseable Content-Range
    let chunk = b"some data";
    let resp = server
        .client
        .patch(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}",
            project_slug, session_id
        )))
        .header("Content-Type", "application/octet-stream")
        .header("Content-Range", "garbage-value")
        .body(chunk.to_vec())
        .send()
        .await
        .expect("Request failed");

    // Content-Range with completely unparseable values on both sides should be rejected
    assert_eq!(
        resp.status(),
        StatusCode::RANGE_NOT_SATISFIABLE,
        "Unparseable Content-Range should be rejected as malformed"
    );
}

// PATCH with Content-Range including "bytes " prefix (standard HTTP format)
#[tokio::test]
async fn upload_content_range_bytes_prefix() {
    let server = TestServer::new().await;
    let user = server
        .signup("BytesPrefix User", "uploadbytesprefix@example.com")
        .await;
    let org = server.create_org(&user, "BytesPrefix Org").await;
    let project = server
        .create_project(&user, &org, "BytesPrefix Project")
        .await;

    let oci_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Start an upload
    let start_resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", project_slug)))
        .header("Authorization", format!("Bearer {}", oci_token))
        .send()
        .await
        .expect("Start upload failed");

    let location = start_resp
        .headers()
        .get("location")
        .expect("Missing location header")
        .to_str()
        .expect("Invalid location header");
    let session_id = extract_session_id(location).expect("Invalid location format");

    // Upload with standard HTTP Content-Range format: "bytes 0-9/*"
    let chunk = b"0123456789";
    let resp = server
        .client
        .patch(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}",
            project_slug, session_id
        )))
        .header("Content-Type", "application/octet-stream")
        .header("Content-Range", format!("bytes 0-{}/*", chunk.len() - 1))
        .body(chunk.to_vec())
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        resp.status(),
        StatusCode::ACCEPTED,
        "Content-Range with 'bytes ' prefix should be accepted"
    );
}

// OPTIONS /v2/{name}/blobs/uploads/{session_id} - CORS preflight
#[tokio::test]
async fn upload_session_options() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .request(
            reqwest::Method::OPTIONS,
            server.api_url("/v2/test-project/blobs/uploads/some-session-id"),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    assert!(resp.headers().contains_key("access-control-allow-origin"));
}

// PATCH with Content-Range where only end is unparseable (e.g., "0-abc") should return 416
#[tokio::test]
async fn upload_partial_content_range_bad_end() {
    let server = TestServer::new().await;
    let user = server
        .signup("BadEnd User", "uploadbadend@example.com")
        .await;
    let org = server.create_org(&user, "BadEnd Org").await;
    let project = server.create_project(&user, &org, "BadEnd Project").await;

    let oci_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Start an upload
    let start_resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", project_slug)))
        .header("Authorization", format!("Bearer {}", oci_token))
        .send()
        .await
        .expect("Start upload failed");

    let location = start_resp
        .headers()
        .get("location")
        .expect("Missing location header")
        .to_str()
        .expect("Invalid location header");
    let session_id = extract_session_id(location).expect("Invalid location format");

    // Upload with partially unparseable Content-Range (valid start, invalid end)
    let chunk = b"some data";
    let resp = server
        .client
        .patch(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}",
            project_slug, session_id
        )))
        .header("Content-Type", "application/octet-stream")
        .header("Content-Range", "0-abc")
        .body(chunk.to_vec())
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        resp.status(),
        StatusCode::RANGE_NOT_SATISFIABLE,
        "Partially unparseable Content-Range (bad end) should return 416"
    );
}

// PATCH with Content-Range where only start is unparseable (e.g., "abc-10") should return 416
#[tokio::test]
async fn upload_partial_content_range_bad_start() {
    let server = TestServer::new().await;
    let user = server
        .signup("BadStart User", "uploadbadstart@example.com")
        .await;
    let org = server.create_org(&user, "BadStart Org").await;
    let project = server.create_project(&user, &org, "BadStart Project").await;

    let oci_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Start an upload
    let start_resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", project_slug)))
        .header("Authorization", format!("Bearer {}", oci_token))
        .send()
        .await
        .expect("Start upload failed");

    let location = start_resp
        .headers()
        .get("location")
        .expect("Missing location header")
        .to_str()
        .expect("Invalid location header");
    let session_id = extract_session_id(location).expect("Invalid location format");

    // Upload with partially unparseable Content-Range (invalid start, valid end)
    let chunk = b"some data";
    let resp = server
        .client
        .patch(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}",
            project_slug, session_id
        )))
        .header("Content-Type", "application/octet-stream")
        .header("Content-Range", "abc-10")
        .body(chunk.to_vec())
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        resp.status(),
        StatusCode::RANGE_NOT_SATISFIABLE,
        "Partially unparseable Content-Range (bad start) should return 416"
    );
}

// =============================================================================
// Concurrent Upload Tests
// =============================================================================

// Start 2 upload sessions to same repo, upload chunks via tokio::join!, complete both, verify both blobs exist
#[tokio::test]
async fn concurrent_uploads_different_sessions() {
    let server = TestServer::new().await;
    let user = server
        .signup("Concurrent User", "concurrentupload@example.com")
        .await;
    let org = server.create_org(&user, "Concurrent Org").await;
    let project = server
        .create_project(&user, &org, "Concurrent Project")
        .await;

    let oci_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Start two upload sessions
    let start1 = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", project_slug)))
        .header("Authorization", format!("Bearer {}", oci_token))
        .send()
        .await
        .expect("Start upload 1 failed");
    assert_eq!(start1.status(), StatusCode::ACCEPTED);
    let location1 = start1
        .headers()
        .get("location")
        .expect("Missing location header 1")
        .to_str()
        .expect("Invalid location header 1")
        .to_owned();
    let session_id1 = extract_session_id(&location1).expect("Invalid location format 1");

    let start2 = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", project_slug)))
        .header("Authorization", format!("Bearer {}", oci_token))
        .send()
        .await
        .expect("Start upload 2 failed");
    assert_eq!(start2.status(), StatusCode::ACCEPTED);
    let location2 = start2
        .headers()
        .get("location")
        .expect("Missing location header 2")
        .to_str()
        .expect("Invalid location header 2")
        .to_owned();
    let session_id2 = extract_session_id(&location2).expect("Invalid location format 2");

    // Upload chunks concurrently via tokio::join!
    let chunk1 = b"concurrent blob data one";
    let chunk2 = b"concurrent blob data two";

    let patch_url1 = server.api_url(&format!(
        "/v2/{}/blobs/uploads/{}",
        project_slug, session_id1
    ));
    let patch_url2 = server.api_url(&format!(
        "/v2/{}/blobs/uploads/{}",
        project_slug, session_id2
    ));

    let (patch_resp1, patch_resp2) = tokio::join!(
        server
            .client
            .patch(patch_url1)
            .header("Content-Type", "application/octet-stream")
            .body(chunk1.to_vec())
            .send(),
        server
            .client
            .patch(patch_url2)
            .header("Content-Type", "application/octet-stream")
            .body(chunk2.to_vec())
            .send()
    );

    assert_eq!(
        patch_resp1.expect("PATCH 1 failed").status(),
        StatusCode::ACCEPTED
    );
    assert_eq!(
        patch_resp2.expect("PATCH 2 failed").status(),
        StatusCode::ACCEPTED
    );

    // Complete both uploads sequentially
    let digest1 = compute_digest(chunk1);
    let complete1 = server
        .client
        .put(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}?digest={}",
            project_slug, session_id1, digest1
        )))
        .send()
        .await
        .expect("Complete upload 1 failed");
    assert_eq!(complete1.status(), StatusCode::CREATED);

    let digest2 = compute_digest(chunk2);
    let complete2 = server
        .client
        .put(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}?digest={}",
            project_slug, session_id2, digest2
        )))
        .send()
        .await
        .expect("Complete upload 2 failed");
    assert_eq!(complete2.status(), StatusCode::CREATED);

    // Verify both blobs exist via HEAD
    let pull_token = server.oci_pull_token(&user, &project);

    let head1 = server
        .client
        .head(server.api_url(&format!("/v2/{}/blobs/{}", project_slug, digest1)))
        .header("Authorization", format!("Bearer {}", pull_token))
        .send()
        .await
        .expect("HEAD blob 1 failed");
    assert_eq!(
        head1.status(),
        StatusCode::OK,
        "Blob 1 should exist after concurrent upload"
    );

    let head2 = server
        .client
        .head(server.api_url(&format!("/v2/{}/blobs/{}", project_slug, digest2)))
        .header("Authorization", format!("Bearer {}", pull_token))
        .send()
        .await
        .expect("HEAD blob 2 failed");
    assert_eq!(
        head2.status(),
        StatusCode::OK,
        "Blob 2 should exist after concurrent upload"
    );
}

// =============================================================================
// Upload Session Expiration Tests
// =============================================================================

// Start a session, advance mock clock past timeout, start another session
// (triggers cleanup), then verify the first session is gone
#[tokio::test]
async fn upload_session_expired() {
    let base_time = chrono::Utc::now().timestamp();
    let mock_time = Arc::new(AtomicI64::new(base_time));
    let time_ref = mock_time.clone();
    let clock =
        bencher_oci_storage::Clock::Custom(Arc::new(move || time_ref.load(Ordering::Relaxed)));

    // 1-second upload timeout
    let server = TestServer::new_with_clock(1, 1_073_741_824, clock).await;
    let user = server
        .signup("Expired User", "uploadexpired@example.com")
        .await;
    let org = server.create_org(&user, "Expired Org").await;
    let project = server.create_project(&user, &org, "Expired Project").await;

    let oci_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Start session A (created_at = base_time)
    let start_a = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", project_slug)))
        .header("Authorization", format!("Bearer {}", oci_token))
        .send()
        .await
        .expect("Start upload A failed");
    assert_eq!(start_a.status(), StatusCode::ACCEPTED);
    let location_a = start_a
        .headers()
        .get("location")
        .expect("Missing location header A")
        .to_str()
        .expect("Invalid location header A")
        .to_owned();
    let session_id_a = extract_session_id(&location_a).expect("Invalid location format A");

    // Advance mock clock past the 1-second timeout (no real sleep!)
    mock_time.fetch_add(2, Ordering::Relaxed);

    // Start session B (triggers spawn_stale_upload_cleanup which sees session A as stale)
    let start_b = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", project_slug)))
        .header("Authorization", format!("Bearer {}", oci_token))
        .send()
        .await
        .expect("Start upload B failed");
    assert_eq!(start_b.status(), StatusCode::ACCEPTED);
    let location_b = start_b
        .headers()
        .get("location")
        .expect("Missing location header B")
        .to_str()
        .expect("Invalid location header B")
        .to_owned();
    let session_id_b = extract_session_id(&location_b).expect("Invalid location format B");

    // Poll for background cleanup task to complete
    // (short intervals — only waiting for tokio::spawn scheduling, not wall-clock staleness)
    let mut cleaned_up = false;
    for _ in 0..1_000 {
        tokio::time::sleep(Duration::from_millis(10)).await;
        let check_a = server
            .client
            .get(server.api_url(&format!(
                "/v2/{}/blobs/uploads/{}",
                project_slug, session_id_a
            )))
            .send()
            .await
            .expect("Check session A failed");
        if check_a.status() == StatusCode::NOT_FOUND {
            cleaned_up = true;
            break;
        }
    }
    assert!(cleaned_up, "Expired session A should be cleaned up");

    // Session B should still be valid (204)
    let check_b = server
        .client
        .get(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}",
            project_slug, session_id_b
        )))
        .send()
        .await
        .expect("Check session B failed");
    assert_eq!(
        check_b.status(),
        StatusCode::NO_CONTENT,
        "Session B should still be valid"
    );
}

// =============================================================================
// Max Body Size Tests
// =============================================================================

// Chunked upload cumulative exceeds max body size
#[tokio::test]
async fn chunked_upload_exceeds_max_body_size() {
    // max_body_size = 100 bytes
    let server = TestServer::new_with_limits(3600, 100).await;
    let user = server
        .signup("ChunkLimit User", "chunklimit@example.com")
        .await;
    let org = server.create_org(&user, "ChunkLimit Org").await;
    let project = server
        .create_project(&user, &org, "ChunkLimit Project")
        .await;

    let oci_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Start an upload
    let start_resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", project_slug)))
        .header("Authorization", format!("Bearer {}", oci_token))
        .send()
        .await
        .expect("Start upload failed");
    let location = start_resp
        .headers()
        .get("location")
        .expect("Missing location header")
        .to_str()
        .expect("Invalid location header")
        .to_owned();
    let session_id = extract_session_id(&location).expect("Invalid location format");

    // First chunk: 60 bytes (within limit)
    let chunk1 = vec![0xAA; 60];
    let resp1 = server
        .client
        .patch(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}",
            project_slug, session_id
        )))
        .header("Content-Type", "application/octet-stream")
        .body(chunk1)
        .send()
        .await
        .expect("Chunk 1 failed");
    assert_eq!(
        resp1.status(),
        StatusCode::ACCEPTED,
        "First 60-byte chunk should be accepted"
    );

    // Second chunk: 60 bytes (cumulative 120 > 100)
    let chunk2 = vec![0xBB; 60];
    let resp2 = server
        .client
        .patch(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}",
            project_slug, session_id
        )))
        .header("Content-Type", "application/octet-stream")
        .body(chunk2)
        .send()
        .await
        .expect("Chunk 2 failed");
    assert_eq!(
        resp2.status(),
        StatusCode::PAYLOAD_TOO_LARGE,
        "Second chunk exceeding max body size should be rejected, got {}",
        resp2.status()
    );
}

// =============================================================================
// Storage Failure Injection Tests
// =============================================================================

// Upload complete after storage directory is deleted
#[tokio::test]
async fn upload_complete_after_storage_deleted() {
    let server = TestServer::new().await;
    let user = server
        .signup("StorageFail User", "storagefailupload@example.com")
        .await;
    let org = server.create_org(&user, "StorageFail Org").await;
    let project = server
        .create_project(&user, &org, "StorageFail Project")
        .await;

    let oci_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Start an upload and patch a chunk
    let start_resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", project_slug)))
        .header("Authorization", format!("Bearer {}", oci_token))
        .send()
        .await
        .expect("Start upload failed");
    let location = start_resp
        .headers()
        .get("location")
        .expect("Missing location header")
        .to_str()
        .expect("Invalid location header")
        .to_owned();
    let session_id = extract_session_id(&location).expect("Invalid location format");

    let chunk = b"data for storage failure test";
    let patch_resp = server
        .client
        .patch(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}",
            project_slug, session_id
        )))
        .header("Content-Type", "application/octet-stream")
        .body(chunk.to_vec())
        .send()
        .await
        .expect("Patch failed");
    assert_eq!(patch_resp.status(), StatusCode::ACCEPTED);

    // Delete the upload session's storage directory
    let upload_dir = server
        .db_path()
        .parent()
        .unwrap()
        .join("oci")
        .join("_uploads")
        .join(&session_id);
    tokio::fs::remove_dir_all(&upload_dir)
        .await
        .expect("Failed to delete upload storage directory");

    // Try to complete the upload - should fail
    let digest = compute_digest(chunk);
    let resp = server
        .client
        .put(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}?digest={}",
            project_slug, session_id, digest
        )))
        .send()
        .await
        .expect("Request failed");

    assert!(
        resp.status().is_client_error() || resp.status().is_server_error(),
        "Upload complete after storage deleted should fail, got {}",
        resp.status()
    );
}

// Verify that rejected data is never written to disk (size check before write)
#[tokio::test]
async fn chunked_upload_size_not_written_on_reject() {
    // max_body_size = 100 bytes
    let server = TestServer::new_with_limits(3600, 100).await;
    let user = server
        .signup("SizeReject User", "sizereject@example.com")
        .await;
    let org = server.create_org(&user, "SizeReject Org").await;
    let project = server
        .create_project(&user, &org, "SizeReject Project")
        .await;

    let oci_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // Start an upload
    let start_resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", project_slug)))
        .header("Authorization", format!("Bearer {}", oci_token))
        .send()
        .await
        .expect("Start upload failed");
    assert_eq!(start_resp.status(), StatusCode::ACCEPTED);
    let location = start_resp
        .headers()
        .get("location")
        .expect("Missing location header")
        .to_str()
        .expect("Invalid location header")
        .to_owned();
    let session_id = extract_session_id(&location).expect("Invalid location format");

    // First chunk: 60 bytes (within limit)
    let chunk1 = vec![0xAA; 60];
    let resp1 = server
        .client
        .patch(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}",
            project_slug, session_id
        )))
        .header("Content-Type", "application/octet-stream")
        .body(chunk1)
        .send()
        .await
        .expect("Chunk 1 failed");
    assert_eq!(
        resp1.status(),
        StatusCode::ACCEPTED,
        "First 60-byte chunk should be accepted"
    );

    // Second chunk: 60 bytes (cumulative 120 > 100, should be rejected)
    let chunk2 = vec![0xBB; 60];
    let resp2 = server
        .client
        .patch(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}",
            project_slug, session_id
        )))
        .header("Content-Type", "application/octet-stream")
        .body(chunk2)
        .send()
        .await
        .expect("Chunk 2 failed");
    assert_eq!(
        resp2.status(),
        StatusCode::PAYLOAD_TOO_LARGE,
        "Second chunk exceeding max body size should be rejected, got {}",
        resp2.status()
    );

    // Verify the upload size is still 60 bytes (rejected data was NOT written)
    let status_resp = server
        .client
        .get(server.api_url(&format!(
            "/v2/{}/blobs/uploads/{}",
            project_slug, session_id
        )))
        .send()
        .await
        .expect("Status request failed");
    assert_eq!(status_resp.status(), StatusCode::NO_CONTENT);

    // The Range header reports the byte range written so far: "0-59" for 60 bytes
    let range = status_resp
        .headers()
        .get("range")
        .expect("Missing range header")
        .to_str()
        .expect("Invalid range header");
    assert_eq!(
        range, "0-59",
        "Upload size should still be 60 bytes (rejected data should not be written), got range: {range}"
    );
}
