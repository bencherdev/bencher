#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args,
    clippy::similar_names,
    clippy::integer_division,
    reason = "integration test file"
)]
//! Integration tests for OCI blob endpoints.

use bencher_api_tests::TestServer;
use bencher_api_tests::helpers::{organizations, projects};
use bencher_api_tests::oci::compute_digest;
use bencher_json::{OrganizationUuid, RunnerUuid};
use bencher_token::OciAction;
use http::StatusCode;
use std::sync::Arc;
use tokio::task::JoinSet;

/// As many blob upload starts as a Docker push of a many-layer image sends at once.
const RACERS: usize = 8;
/// Long enough for an upload start to pass its reads and queue on the API's writer.
const QUEUE: std::time::Duration = std::time::Duration::from_secs(1);

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

// POST /v2/{name}/blobs/uploads - Start upload (public token, unclaimed project)
#[tokio::test]
async fn blob_upload_start_unclaimed() {
    let server = TestServer::new().await;
    let slug = "unclaimed-blob-project";

    let public_token = server.oci_public_token(slug, &[OciAction::Push]);

    // Push to a new project slug (will auto-create unclaimed project)
    let resp = server
        .client
        .post(server.api_url(&format!("/v2/{slug}/blobs/uploads")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&public_token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    assert!(resp.headers().contains_key("location"));
}

// POST /v2/{name}/blobs/uploads - Start upload with no Bearer token → 401
#[tokio::test]
async fn blob_upload_no_token_rejected() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .post(server.api_url("/v2/some-project/blobs/uploads"))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert!(resp.headers().contains_key("www-authenticate"));
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

    // First, create an unclaimed project by pushing with a public token
    let unclaimed_slug = "unclaimed-to-claim-project";
    let public_token = server.oci_public_token(unclaimed_slug, &[OciAction::Push]);

    let create_resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", unclaimed_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&public_token),
        )
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

    // Verify the project is now claimed by trying a public-token push again
    // It should now be rejected since the org was claimed
    let public_resp = server
        .client
        .post(server.api_url(&format!("/v2/{}/blobs/uploads", unclaimed_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&public_token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(public_resp.status(), StatusCode::UNAUTHORIZED);
}

// A structurally valid but wrongly-signed JWT must be rejected with 401.
#[tokio::test]
async fn blob_upload_tampered_token_rejected() {
    let server = TestServer::new().await;
    let unclaimed_slug = "tampered-token-test-project";

    // Get a valid token, then tamper with it to create a structurally valid
    // but wrongly-signed JWT (flip a char in the middle of the signature).
    // We must preserve valid JWT structure (3 dot-separated base64url parts)
    // so that Jwt::parse() succeeds.
    let user = server.signup("Tampered User", "tampered@example.com").await;
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
        "Tampered JWT must be rejected with 401"
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

// Concurrent upload starts by one user to a new project slug all start, and create the project once,
// in the user's organization.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn blob_upload_starts_at_once_create_the_project_once() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "raceadmin@example.com").await;
    let user = server.signup("Race User", "raceuser@example.com").await;
    let slug = "raced-project";
    let token = server.oci_token(&user, slug, &[OciAction::Push]);

    let statuses = start_uploads_at_once(&server, vec![(slug, token); RACERS]).await;

    assert_eq!(statuses, vec![StatusCode::ACCEPTED; RACERS]);
    assert_eq!(projects(&server), vec![(slug.to_owned(), user.uuid.into())]);
    assert_eq!(
        organizations(&server),
        vec![OrganizationUuid::from(admin.uuid), user.uuid.into()]
    );
}

// Concurrent anonymous upload starts to a new project slug all start, and create the project and
// its organization once.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn blob_upload_starts_at_once_unclaimed_create_the_project_once() {
    let server = TestServer::new().await;
    let slug = "raced-unclaimed-project";
    let token = server.oci_public_token(slug, &[OciAction::Push]);

    let statuses = start_uploads_at_once(&server, vec![(slug, token); RACERS]).await;

    assert_eq!(statuses, vec![StatusCode::ACCEPTED; RACERS]);
    let organizations = organizations(&server);
    assert_eq!(organizations.len(), 1);
    assert_eq!(projects(&server), vec![(slug.to_owned(), organizations[0])]);
}

// A user who claimed an organization at signup has no organization of its own. Concurrent upload
// starts by that user to a new project slug, all queued on the writer before any creates the
// organization, create that organization once and the project once, with the default rate limits.
// Only the one that creates the organization counts toward the user's organization creation limit.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn blob_upload_starts_at_once_create_the_user_organization_once() {
    let server = TestServer::new_with_default_rate_limits().await;
    let admin = server.signup("Admin", "raceorgadmin@example.com").await;
    let claimed_slug = "claimed-at-signup";
    let public_token = server.oci_public_token(claimed_slug, &[OciAction::Push]);
    let statuses = start_uploads_at_once(&server, vec![(claimed_slug, public_token)]).await;
    assert_eq!(statuses, vec![StatusCode::ACCEPTED]);
    let claimed = organizations(&server)[1];
    let user = server
        .signup_with_claim("Claim User", "raceorguser@example.com", Some(claimed))
        .await;
    let slug = "raced-first-project";
    let token = server.oci_token(&user, slug, &[OciAction::Push]);

    let statuses =
        start_uploads_behind_the_writer(&server, vec![vec![(slug, token); RACERS]]).await;

    assert_eq!(statuses, vec![StatusCode::ACCEPTED; RACERS]);
    assert_eq!(
        projects(&server),
        vec![
            (claimed_slug.to_owned(), claimed),
            (slug.to_owned(), user.uuid.into())
        ]
    );
    assert_eq!(
        organizations(&server),
        vec![
            OrganizationUuid::from(admin.uuid),
            claimed,
            user.uuid.into()
        ]
    );
    // The organization creation limit allows 2 a minute, and the race counted once.
    let mut statuses = Vec::new();
    for i in 0..3 {
        let status = server
            .client
            .post(server.api_url("/v0/organizations"))
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(&user.token),
            )
            .json(&serde_json::json!({ "name": format!("After The Race {i}") }))
            .send()
            .await
            .expect("Request failed")
            .status();
        statuses.push(status);
        if status != StatusCode::CREATED {
            break;
        }
    }
    assert_eq!(
        statuses,
        vec![StatusCode::CREATED, StatusCode::TOO_MANY_REQUESTS]
    );
}

// A user who left their own organization still has an organization with their UUID, which they
// cannot use. An upload start by that user to a new project slug is refused as a conflict.
#[tokio::test]
async fn blob_upload_start_by_a_user_who_left_their_organization_conflicts() {
    let server = TestServer::new().await;
    let _admin = server.signup("Admin", "leftadmin@example.com").await;
    let user = server.signup("Left User", "leftuser@example.com").await;
    let resp = server
        .client
        .delete(server.api_url(&format!(
            "/v0/organizations/{uuid}/members/{uuid}",
            uuid = user.uuid
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    let slug = "left-project";
    let token = server.oci_token(&user, slug, &[OciAction::Push]);

    let statuses = start_uploads_at_once(&server, vec![(slug, token)]).await;

    assert_eq!(statuses, vec![StatusCode::CONFLICT]);
    assert_eq!(projects(&server), Vec::new());
}

// Concurrent upload starts to a new project slug by a user one project short of its organization's
// creation limit all start: only the one that creates the project counts toward the limit.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn blob_upload_starts_at_once_at_the_creation_limit_create_the_project_once() {
    let server = TestServer::new_with_creation_limits(u32::MAX, 1).await;
    let _admin = server.signup("Admin", "racelimitadmin@example.com").await;
    let user = server.signup("Limit User", "racelimit@example.com").await;
    let slug = "raced-last-project";
    let token = server.oci_token(&user, slug, &[OciAction::Push]);

    let statuses = start_uploads_at_once(&server, vec![(slug, token); RACERS]).await;

    assert_eq!(statuses, vec![StatusCode::ACCEPTED; RACERS]);
    assert_eq!(projects(&server), vec![(slug.to_owned(), user.uuid.into())]);
}

// An anonymous upload start that the creation limit refuses leaves no project and no organization.
#[tokio::test]
async fn blob_upload_start_unclaimed_refused_by_the_creation_limit_creates_nothing() {
    let server = TestServer::new_with_creation_limits(0, u32::MAX).await;
    let slug = "refused-unclaimed-project";
    let token = server.oci_public_token(slug, &[OciAction::Push]);

    let statuses = start_uploads_at_once(&server, vec![(slug, token)]).await;

    assert_eq!(statuses, vec![StatusCode::TOO_MANY_REQUESTS]);
    assert_eq!(organizations(&server), Vec::new());
    assert_eq!(projects(&server), Vec::new());
}

// Two users race to create the same project slug. The project lands in one user's organization:
// that user's upload starts all start, and the other user's are all denied, as they would be on
// the project once it exists.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn blob_upload_starts_at_once_by_two_users_create_the_project_for_one() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "racetwoadmin@example.com").await;
    let first = server.signup("First User", "racefirst@example.com").await;
    let second = server.signup("Second User", "racesecond@example.com").await;
    let slug = "contested-project";
    let racers = [&first, &second]
        .into_iter()
        .cycle()
        .take(RACERS)
        .collect::<Vec<_>>();
    let uploads = racers
        .iter()
        .map(|user| (slug, server.oci_token(user, slug, &[OciAction::Push])))
        .collect();

    let statuses = start_uploads_at_once(&server, uploads).await;

    let projects = projects(&server);
    assert_eq!(projects.len(), 1);
    let (project_slug, owner) = &projects[0];
    assert_eq!(project_slug, slug);
    let expected = racers
        .iter()
        .map(|user| {
            if OrganizationUuid::from(user.uuid) == *owner {
                StatusCode::ACCEPTED
            } else {
                StatusCode::FORBIDDEN
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(statuses, expected);
    assert_eq!(
        organizations(&server),
        vec![
            OrganizationUuid::from(admin.uuid),
            first.uuid.into(),
            second.uuid.into()
        ]
    );
}

// A user and an anonymous pusher race to create each of several project slugs, the user's upload
// starts queued on the writer before the anonymous ones, after both passed their reads. Each project
// is created once, in the user's organization. The anonymous upload starts are all refused, as they
// would be on a project that exists, and leave no organization behind.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn blob_upload_starts_at_once_by_a_user_and_anonymous_create_each_project_once() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "racemixedadmin@example.com").await;
    let user = server.signup("Mixed User", "racemixed@example.com").await;
    let slugs = (0..RACERS)
        .map(|i| format!("mixed-project-{i}"))
        .collect::<Vec<_>>();
    let user_uploads = slugs
        .iter()
        .map(|slug| {
            (
                slug.as_str(),
                server.oci_token(&user, slug, &[OciAction::Push]),
            )
        })
        .collect();
    let public_uploads = slugs
        .iter()
        .map(|slug| {
            (
                slug.as_str(),
                server.oci_public_token(slug, &[OciAction::Push]),
            )
        })
        .collect();

    let statuses =
        start_uploads_behind_the_writer(&server, vec![user_uploads, public_uploads]).await;

    let expected = [StatusCode::ACCEPTED; RACERS]
        .into_iter()
        .chain([StatusCode::UNAUTHORIZED; RACERS])
        .collect::<Vec<_>>();
    assert_eq!(statuses, expected);
    let mut projects = projects(&server);
    projects.sort();
    let expected_projects = slugs
        .iter()
        .map(|slug| (slug.clone(), OrganizationUuid::from(user.uuid)))
        .collect::<Vec<_>>();
    assert_eq!(projects, expected_projects);
    assert_eq!(
        organizations(&server),
        vec![OrganizationUuid::from(admin.uuid), user.uuid.into()]
    );
}

/// Start an upload with each repository and token at once, and return each status in order.
async fn start_uploads_at_once(
    server: &TestServer,
    uploads: Vec<(&str, String)>,
) -> Vec<StatusCode> {
    let mut starts = JoinSet::new();
    for (i, (repository, token)) in uploads.into_iter().enumerate() {
        spawn_upload_start(&mut starts, server, i, repository, &token);
    }
    statuses_in_order(starts).await
}

/// Start an upload with each repository and token while holding the API's writer, each batch once
/// the batch before it has queued on the writer, then let the writer go, and return each status in
/// order.
async fn start_uploads_behind_the_writer(
    server: &TestServer,
    batches: Vec<Vec<(&str, String)>>,
) -> Vec<StatusCode> {
    let writer = Arc::clone(&server.context().database.connection)
        .lock_owned()
        .await;
    let mut starts = JoinSet::new();
    let mut i = 0;
    for batch in batches {
        for (repository, token) in batch {
            spawn_upload_start(&mut starts, server, i, repository, &token);
            i += 1;
        }
        tokio::time::sleep(QUEUE).await;
    }
    drop(writer);
    statuses_in_order(starts).await
}

#[expect(clippy::expect_used, reason = "test helper sending requests")]
fn spawn_upload_start(
    starts: &mut JoinSet<(usize, StatusCode)>,
    server: &TestServer,
    i: usize,
    repository: &str,
    token: &str,
) {
    let request = server
        .client
        .post(server.api_url(&format!("/v2/{repository}/blobs/uploads")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(token),
        );
    starts.spawn(async move { (i, request.send().await.expect("Request failed").status()) });
}

async fn statuses_in_order(starts: JoinSet<(usize, StatusCode)>) -> Vec<StatusCode> {
    let mut statuses = starts.join_all().await;
    statuses.sort_by_key(|(i, _)| *i);
    statuses.into_iter().map(|(_, status)| status).collect()
}

// Monolithic upload (PUT /blobs/uploads?digest=) to unclaimed project with public token
#[tokio::test]
async fn blob_monolithic_upload_unclaimed() {
    let server = TestServer::new().await;
    let slug = "unclaimed-monolithic-project";

    let blob_data = b"unclaimed monolithic blob content";
    let digest = compute_digest(blob_data);
    let public_token = server.oci_public_token(slug, &[OciAction::Push]);

    // Push to a new project slug (will auto-create unclaimed project)
    let resp = server
        .client
        .put(server.api_url(&format!("/v2/{slug}/blobs/uploads?digest={digest}")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&public_token),
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
        .join("registry")
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

    // Token is valid but lacks pull scope — 403 FORBIDDEN
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
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

    // Token is valid but scoped to wrong repository — 403 FORBIDDEN
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
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

    // Token is valid but lacks pull action — 403 FORBIDDEN
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "Runner token with only Push action should be rejected for pull"
    );
}

// =============================================================================
// StreamingBody / Large Upload Tests
// =============================================================================

// Monolithic upload with body larger than old 1 MB default succeeds
#[tokio::test]
async fn blob_monolithic_upload_large_body() {
    let server = TestServer::new().await;
    let user = server
        .signup("LargeMono User", "largemono@example.com")
        .await;
    let org = server.create_org(&user, "LargeMono Org").await;
    let project = server
        .create_project(&user, &org, "LargeMono Project")
        .await;

    let oci_token = server.oci_push_token(&user, &project);
    let project_slug: &str = project.slug.as_ref();

    // 2 MiB blob exceeds the old 1 MB default
    let blob_data = vec![0xDD; 2 * 1024 * 1024];
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
        StatusCode::CREATED,
        "Large monolithic upload should succeed with per-endpoint limit, got {}",
        resp.status()
    );
    assert!(resp.headers().contains_key("location"));
    assert!(resp.headers().contains_key("docker-content-digest"));

    // Verify blob exists via HEAD
    let pull_token = server.oci_pull_token(&user, &project);
    let head_resp = server
        .client
        .head(server.api_url(&format!("/v2/{}/blobs/{}", project_slug, digest)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&pull_token),
        )
        .send()
        .await
        .expect("HEAD request failed");

    assert_eq!(head_resp.status(), StatusCode::OK);

    // Verify content via GET
    let get_resp = server
        .client
        .get(server.api_url(&format!("/v2/{}/blobs/{}", project_slug, digest)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&pull_token),
        )
        .send()
        .await
        .expect("GET request failed");

    assert_eq!(get_resp.status(), StatusCode::OK);
    let body = get_resp.bytes().await.expect("Failed to read body");
    assert_eq!(body.len(), 2 * 1024 * 1024);
}
