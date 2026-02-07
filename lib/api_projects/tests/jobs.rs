#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::redundant_test_prefix
)]
//! Integration tests for project job endpoints.
//!
//! Note: Jobs are created when reports are submitted, so most tests
//! verify correct behavior with empty job lists and proper error handling.

use bencher_api_tests::TestServer;
use bencher_json::runner::JsonJobs;
use http::StatusCode;

// GET /v0/projects/{project}/jobs - list jobs (empty)
#[tokio::test]
async fn test_jobs_list_empty() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "joblist@example.com").await;
    let org = server.create_org(&user, "Job Org").await;
    let project = server.create_project(&user, &org, "Job Project").await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project_slug}/jobs")))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let jobs: JsonJobs = resp.json().await.expect("Failed to parse response");
    // New project should have no jobs
    assert!(jobs.0.is_empty());
}

// GET /v0/projects/{project}/jobs - with pagination
#[tokio::test]
async fn test_jobs_list_with_pagination() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "jobpage@example.com").await;
    let org = server.create_org(&user, "Job Page Org").await;
    let project = server.create_project(&user, &org, "Job Page Project").await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{project_slug}/jobs?per_page=10&page=1"
        )))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let jobs: JsonJobs = resp.json().await.expect("Failed to parse response");
    assert!(jobs.0.is_empty());
}

// GET /v0/projects/{project}/jobs - with status filter
#[tokio::test]
async fn test_jobs_list_with_status_filter() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "jobstatus@example.com").await;
    let org = server.create_org(&user, "Job Status Org").await;
    let project = server
        .create_project(&user, &org, "Job Status Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project_slug}/jobs?status=pending")))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let jobs: JsonJobs = resp.json().await.expect("Failed to parse response");
    assert!(jobs.0.is_empty());
}

// GET /v0/projects/{project}/jobs - with ascending order
#[tokio::test]
async fn test_jobs_list_ascending_order() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "jobasc@example.com").await;
    let org = server.create_org(&user, "Job Asc Org").await;
    let project = server.create_project(&user, &org, "Job Asc Project").await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project_slug}/jobs?direction=asc")))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
}

// GET /v0/projects/{project}/jobs/{job} - not found
#[tokio::test]
async fn test_jobs_get_not_found() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "jobnotfound@example.com").await;
    let org = server.create_org(&user, "Job NotFound Org").await;
    let project = server
        .create_project(&user, &org, "Job NotFound Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{project_slug}/jobs/00000000-0000-0000-0000-000000000000"
        )))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// GET /v0/projects/{project}/jobs - public project, no auth
#[tokio::test]
async fn test_jobs_list_public_no_auth() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "jobpublic@example.com").await;
    let org = server.create_org(&user, "Job Public Org").await;
    let project = server
        .create_project(&user, &org, "Job Public Project")
        .await;

    // Projects are public by default
    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project_slug}/jobs")))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let jobs: JsonJobs = resp.json().await.expect("Failed to parse response");
    assert!(jobs.0.is_empty());
}

// GET /v0/projects/{project}/jobs/{job} - public project, no auth, not found
#[tokio::test]
async fn test_jobs_get_public_no_auth_not_found() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "jobpub2@example.com").await;
    let org = server.create_org(&user, "Job Public2 Org").await;
    let project = server
        .create_project(&user, &org, "Job Public2 Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{project_slug}/jobs/00000000-0000-0000-0000-000000000000"
        )))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// GET /v0/projects/{project}/jobs - nonexistent project
#[tokio::test]
async fn test_jobs_list_project_not_found() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "jobnoproj@example.com").await;

    let resp = server
        .client
        .get(server.api_url("/v0/projects/nonexistent-project/jobs"))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// GET /v0/projects/{project}/jobs - X-Total-Count header present
#[tokio::test]
async fn test_jobs_list_total_count_header() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "jobtotal@example.com").await;
    let org = server.create_org(&user, "Job Total Org").await;
    let project = server
        .create_project(&user, &org, "Job Total Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project_slug}/jobs")))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    // Check that X-Total-Count header is present
    let total_count = resp.headers().get("X-Total-Count");
    assert!(total_count.is_some());
    assert_eq!(total_count.unwrap().to_str().unwrap(), "0");
}

// GET /v0/projects/{project}/jobs - using project UUID instead of slug
#[tokio::test]
async fn test_jobs_list_by_uuid() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "jobuuid@example.com").await;
    let org = server.create_org(&user, "Job UUID Org").await;
    let project = server.create_project(&user, &org, "Job UUID Project").await;

    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}/jobs", project.uuid)))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
}

// GET /v0/projects/{project}/jobs - private project, no auth, should be denied
#[tokio::test]
async fn test_private_project_jobs_denied_unauthenticated() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "jobprivate@example.com").await;
    let org = server.create_org(&user, "Job Private Org").await;
    let project = server
        .create_project(&user, &org, "Job Private Project")
        .await;

    // Make the project private by updating visibility directly in the database
    {
        use bencher_schema::schema;
        use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};

        let mut conn = server.db_conn();
        diesel::update(schema::project::table.filter(schema::project::uuid.eq(project.uuid)))
            .set(schema::project::visibility.eq(1_i32))
            .execute(&mut conn)
            .expect("Failed to update project visibility");
    }

    // Request jobs without auth - should be denied
    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project_slug}/jobs")))
        .send()
        .await
        .expect("Request failed");

    // Private project should deny unauthenticated access
    assert!(
        resp.status() == StatusCode::NOT_FOUND || resp.status() == StatusCode::FORBIDDEN,
        "Expected NOT_FOUND or FORBIDDEN for private project, got {}",
        resp.status()
    );
}
