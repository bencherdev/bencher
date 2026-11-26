#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for the OCI error body that every registry error carries.

use bencher_api_tests::oci::{compute_digest, create_oci_manifest, oci_error_codes, oci_errors};
use bencher_api_tests::{LogCapture, TestProject, TestServer, TestUser};
use bencher_json::system::config::JsonRateLimiting;
use bencher_schema::context::RateLimiting;
use bencher_token::OciAction;
use http::StatusCode;
use serde_json::json;

// GET /v2/ without a token: 401 UNAUTHORIZED with a challenge
#[tokio::test]
async fn oci_error_unauthorized() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .get(server.api_url("/v2/"))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let www_authenticate = resp
        .headers()
        .get(http::header::WWW_AUTHENTICATE)
        .and_then(|value| value.to_str().ok())
        .expect("Missing WWW-Authenticate header")
        .to_owned();
    assert!(
        www_authenticate.starts_with("Bearer realm="),
        "{www_authenticate}"
    );
    assert_eq!(oci_error_codes(resp).await, ["UNAUTHORIZED"]);
}

// GET /v2/{name}/blobs/{digest} with a push-only token: 403 DENIED
#[tokio::test]
async fn oci_error_denied() {
    let server = TestServer::new().await;
    let (user, project) = seed_project(&server, "denied").await;
    let push_token = server.oci_push_token(&user, &project);
    let digest = server
        .upload_blob(project.slug.as_ref(), &push_token, b"denied blob")
        .await;

    let resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/blobs/{digest}", project.slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&push_token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert_eq!(oci_error_codes(resp).await, ["DENIED"]);
}

// GET /v2/{name}/blobs/{ref} with a ref that is not a digest: 400 DIGEST_INVALID
#[tokio::test]
async fn oci_error_digest_invalid() {
    let server = TestServer::new().await;
    let (user, project) = seed_project(&server, "digest").await;
    let pull_token = server.oci_pull_token(&user, &project);

    let resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/blobs/not-a-digest", project.slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&pull_token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert_eq!(oci_error_codes(resp).await, ["DIGEST_INVALID"]);
}

// GET /v2/{name}/tags/list with a name that is neither a UUID nor a slug, which the path
// extractor rejects before the handler runs: 400 UNSUPPORTED
#[tokio::test]
async fn oci_error_extractor() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .get(server.api_url("/v2/Not%20A%20Name/tags/list"))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert_eq!(oci_error_codes(resp).await, ["UNSUPPORTED"]);
}

// GET /v2/{name}/manifests/{tag} for a tag that was never pushed: 404 MANIFEST_UNKNOWN
#[tokio::test]
async fn oci_error_manifest_unknown() {
    let server = TestServer::new().await;
    let (user, project) = seed_project(&server, "manifest").await;
    let pull_token = server.oci_pull_token(&user, &project);

    let resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/manifests/missing", project.slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&pull_token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    assert_eq!(oci_error_codes(resp).await, ["MANIFEST_UNKNOWN"]);
}

// GET /v2/{name}/tags/list for a repository that does not exist: 404 NAME_UNKNOWN
#[tokio::test]
async fn oci_error_name_unknown() {
    let server = TestServer::new().await;
    let public_token = server.oci_public_token("no-such-project", &[OciAction::Pull]);

    let resp = server
        .client
        .get(server.api_url("/v2/no-such-project/tags/list"))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&public_token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    assert_eq!(oci_error_codes(resp).await, ["NAME_UNKNOWN"]);
}

// GET /v2/{name}/blobs/{ref}/{session_id} with a ref other than `uploads`: 404 BLOB_UPLOAD_UNKNOWN
#[tokio::test]
async fn oci_error_upload_path_unknown() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .get(server.api_url("/v2/some-project/blobs/downloads/some-session"))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    assert_eq!(oci_error_codes(resp).await, ["BLOB_UPLOAD_UNKNOWN"]);
}

// GET /v2/{name}/blobs/uploads without a session: 405 UNSUPPORTED
#[tokio::test]
async fn oci_error_get_uploads_unsupported() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .get(server.api_url("/v2/some-project/blobs/uploads"))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(oci_error_codes(resp).await, ["UNSUPPORTED"]);
}

// DELETE /v2/{name}/blobs/uploads without a session: 405 UNSUPPORTED
#[tokio::test]
async fn oci_error_delete_uploads_unsupported() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .delete(server.api_url("/v2/some-project/blobs/uploads"))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(oci_error_codes(resp).await, ["UNSUPPORTED"]);
}

// POST /v2/{name}/blobs/{digest}: 405 UNSUPPORTED
#[tokio::test]
async fn oci_error_post_blob_unsupported() {
    let server = TestServer::new().await;
    let digest = compute_digest(b"post blob");

    let resp = server
        .client
        .post(server.api_url(&format!("/v2/some-project/blobs/{digest}")))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(oci_error_codes(resp).await, ["UNSUPPORTED"]);
}

// PUT /v2/{name}/blobs/{digest}?digest=: 405 UNSUPPORTED
#[tokio::test]
async fn oci_error_put_blob_unsupported() {
    let server = TestServer::new().await;
    let digest = compute_digest(b"put blob");

    let resp = server
        .client
        .put(server.api_url(&format!("/v2/some-project/blobs/{digest}?digest={digest}")))
        .body(b"put blob".to_vec())
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(oci_error_codes(resp).await, ["UNSUPPORTED"]);
}

// PUT /v2/{name}/blobs/uploads?digest= past the size limit: 413 SIZE_INVALID
#[tokio::test]
async fn oci_error_size_invalid() {
    let server = TestServer::new_with_limits(3600, 100).await;
    let (user, project) = seed_project(&server, "size").await;
    let push_token = server.oci_push_token(&user, &project);
    let blob = vec![0xCC; 200];
    let digest = compute_digest(&blob);

    let resp = server
        .client
        .put(server.api_url(&format!(
            "/v2/{}/blobs/uploads?digest={digest}",
            project.slug
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&push_token),
        )
        .body(blob)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(oci_error_codes(resp).await, ["SIZE_INVALID"]);
}

// PATCH /v2/{name}/blobs/uploads/{session_id} out of order: 416 BLOB_UPLOAD_INVALID
#[tokio::test]
async fn oci_error_range_not_satisfiable() {
    let server = TestServer::new().await;
    let (user, project) = seed_project(&server, "range").await;
    let push_token = server.oci_push_token(&user, &project);

    let start = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", project.slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&push_token),
        )
        .send()
        .await
        .expect("Start upload failed");
    assert_eq!(start.status(), StatusCode::ACCEPTED);
    let location = start
        .headers()
        .get(http::header::LOCATION)
        .and_then(|value| value.to_str().ok())
        .expect("Missing Location header")
        .to_owned();

    let resp = server
        .client
        .patch(server.api_url(&location))
        .header("Content-Range", "100-108")
        .body(b"some data".to_vec())
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::RANGE_NOT_SATISFIABLE);
    let header = |name: &str| {
        resp.headers()
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned)
    };
    assert_eq!(header("location"), Some(location.clone()));
    assert_eq!(header("range").as_deref(), Some("0-0"));
    assert_eq!(
        header("docker-upload-uuid").as_deref(),
        location.rsplit('/').next()
    );
    assert_eq!(oci_error_codes(resp).await, ["BLOB_UPLOAD_INVALID"]);
}

// GET /v2/ past the public request limit: 429 TOOMANYREQUESTS with its CORS headers
#[tokio::test]
async fn oci_error_too_many_requests() {
    let rate_limiting: JsonRateLimiting =
        serde_json::from_value(json!({"public": {"requests": {"minute": 1}}}))
            .expect("Invalid rate limiting");
    let server = TestServer::new_with_rate_limiting(RateLimiting::from(rate_limiting)).await;
    let public_token = server.oci_public_token("any-project", &[OciAction::Pull]);
    let base = || {
        server
            .client
            .get(server.api_url("/v2/"))
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(&public_token),
            )
            .header("X-Forwarded-For", "203.0.113.7")
            .send()
    };

    assert_eq!(
        base().await.expect("Request failed").status(),
        StatusCode::OK
    );
    let resp = base().await.expect("Request failed");

    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        resp.headers()
            .get(http::header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .and_then(|value| value.to_str().ok()),
        Some("*")
    );
    assert_eq!(oci_error_codes(resp).await, ["TOOMANYREQUESTS"]);
}

// GET /v2/{name}/manifests/{digest} for a stored manifest that no longer parses:
// 500 UNKNOWN with the generic message, and the cause only in the log
#[tokio::test]
async fn oci_error_internal() {
    let capture = LogCapture::default();
    let server = TestServer::new_with_log(capture.logger()).await;
    let (user, project) = seed_project(&server, "internal").await;
    let push_token = server.oci_push_token(&user, &project);
    let config_digest = server
        .upload_blob(project.slug.as_ref(), &push_token, b"internal config")
        .await;
    let layer_digest = server
        .upload_blob(project.slug.as_ref(), &push_token, b"internal layer")
        .await;
    let manifest = create_oci_manifest(&config_digest, &layer_digest);
    let manifest_digest = compute_digest(manifest.as_bytes());
    let put = server
        .client
        .put(server.api_url(&format!("/v2/{}/manifests/latest", project.slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&push_token),
        )
        .header("Content-Type", "application/vnd.oci.image.manifest.v1+json")
        .body(manifest)
        .send()
        .await
        .expect("Manifest upload failed");
    assert_eq!(put.status(), StatusCode::CREATED);

    let hex = manifest_digest
        .strip_prefix("sha256:")
        .expect("Invalid digest");
    let stored = server
        .db_path()
        .parent()
        .expect("Missing database directory")
        .join("registry")
        .join(project.uuid.to_string())
        .join("manifests")
        .join("sha256")
        .join(hex);
    tokio::fs::write(&stored, b"not a manifest")
        .await
        .expect("Failed to overwrite the stored manifest");

    let pull_token = server.oci_pull_token(&user, &project);
    let resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/manifests/{manifest_digest}", project.slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&pull_token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        oci_errors(resp).await,
        [("UNKNOWN".to_owned(), "Internal Server Error".to_owned())]
    );
    assert!(
        capture
            .lines()
            .iter()
            .any(|line| line.contains("Failed to parse stored manifest")),
        "The internal message is missing from the log"
    );
}

async fn seed_project(server: &TestServer, name: &str) -> (TestUser, TestProject) {
    let user = server
        .signup("Error User", &format!("oci-error-{name}@example.com"))
        .await;
    let org = server.create_org(&user, &format!("Error {name} Org")).await;
    let project = server
        .create_project(&user, &org, &format!("Error {name} Project"))
        .await;
    (user, project)
}
