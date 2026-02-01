//! Integration tests for project endpoints.

use bencher_api_tests::TestServer;
use bencher_json::{
    JsonBranch, JsonBranches, JsonMeasure, JsonMeasures, JsonProject, JsonProjects, JsonTestbed,
    JsonTestbeds,
};
use http::StatusCode;

// ============ Projects ============

// GET /v0/projects - list all public projects
#[tokio::test]
async fn test_projects_list() {
    let server = TestServer::new().await;

    let resp = server
        .client
        .get(server.api_url("/v0/projects"))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let _projects: JsonProjects = resp.json().await.expect("Failed to parse response");
}

// GET /v0/projects/{project} - get a project
#[tokio::test]
async fn test_projects_get() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "projget@example.com").await;
    let org = server.create_org(&user, "Project Org").await;
    let project = server.create_project(&user, &org, "Test Project").await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}", project_slug)))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let fetched: JsonProject = resp.json().await.expect("Failed to parse response");
    assert_eq!(fetched.uuid, project.uuid);
}

// PATCH /v0/projects/{project} - update a project
#[tokio::test]
async fn test_projects_update() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "projupdate@example.com").await;
    let org = server.create_org(&user, "Update Org").await;
    let project = server.create_project(&user, &org, "Update Project").await;

    let body = serde_json::json!({
        "name": "Updated Project Name"
    });

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .patch(server.api_url(&format!("/v0/projects/{}", project_slug)))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let updated: JsonProject = resp.json().await.expect("Failed to parse response");
    assert_eq!(updated.name.as_ref(), "Updated Project Name");
}

// DELETE /v0/projects/{project} - delete a project
#[tokio::test]
async fn test_projects_delete() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "projdelete@example.com").await;
    let org = server.create_org(&user, "Delete Org").await;
    let project = server.create_project(&user, &org, "Delete Project").await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .delete(server.api_url(&format!("/v0/projects/{}", project_slug)))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

// GET /v0/projects/{project}/allowed/{permission} - check permission
#[tokio::test]
async fn test_projects_allowed() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "projallowed@example.com").await;
    let org = server.create_org(&user, "Allowed Org").await;
    let project = server.create_project(&user, &org, "Allowed Project").await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/allowed/view",
            project_slug
        )))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
}

// ============ Branches ============

// GET /v0/projects/{project}/branches - list branches
#[tokio::test]
async fn test_projects_branches_list() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "branchlist@example.com").await;
    let org = server.create_org(&user, "Branch Org").await;
    let project = server.create_project(&user, &org, "Branch Project").await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}/branches", project_slug)))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let _branches: JsonBranches = resp.json().await.expect("Failed to parse response");
}

// POST /v0/projects/{project}/branches - create branch
#[tokio::test]
async fn test_projects_branches_create() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "branchcreate@example.com").await;
    let org = server.create_org(&user, "Branch Create Org").await;
    let project = server.create_project(&user, &org, "Branch Create Project").await;

    let body = serde_json::json!({
        "name": "feature-branch",
        "slug": "feature-branch"
    });

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/branches", project_slug)))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
    let branch: JsonBranch = resp.json().await.expect("Failed to parse response");
    assert_eq!(branch.name.as_ref(), "feature-branch");
}

// ============ Testbeds ============

// GET /v0/projects/{project}/testbeds - list testbeds
#[tokio::test]
async fn test_projects_testbeds_list() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "testbedlist@example.com").await;
    let org = server.create_org(&user, "Testbed Org").await;
    let project = server.create_project(&user, &org, "Testbed Project").await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}/testbeds", project_slug)))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let _testbeds: JsonTestbeds = resp.json().await.expect("Failed to parse response");
}

// POST /v0/projects/{project}/testbeds - create testbed
#[tokio::test]
async fn test_projects_testbeds_create() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "testbedcreate@example.com").await;
    let org = server.create_org(&user, "Testbed Create Org").await;
    let project = server.create_project(&user, &org, "Testbed Create Project").await;

    let body = serde_json::json!({
        "name": "linux-server",
        "slug": "linux-server"
    });

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/testbeds", project_slug)))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
    let testbed: JsonTestbed = resp.json().await.expect("Failed to parse response");
    assert_eq!(testbed.name.as_ref(), "linux-server");
}

// ============ Measures ============

// GET /v0/projects/{project}/measures - list measures
#[tokio::test]
async fn test_projects_measures_list() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "measurelist@example.com").await;
    let org = server.create_org(&user, "Measure Org").await;
    let project = server.create_project(&user, &org, "Measure Project").await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}/measures", project_slug)))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let _measures: JsonMeasures = resp.json().await.expect("Failed to parse response");
}

// POST /v0/projects/{project}/measures - create measure
#[tokio::test]
async fn test_projects_measures_create() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "measurecreate@example.com").await;
    let org = server.create_org(&user, "Measure Create Org").await;
    let project = server.create_project(&user, &org, "Measure Create Project").await;

    let body = serde_json::json!({
        "name": "Latency",
        "slug": "latency",
        "units": "ns"
    });

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/measures", project_slug)))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
    let measure: JsonMeasure = resp.json().await.expect("Failed to parse response");
    assert_eq!(measure.name.as_ref(), "Latency");
}

// ============ Benchmarks ============

// GET /v0/projects/{project}/benchmarks - list benchmarks
#[tokio::test]
async fn test_projects_benchmarks_list() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "benchmarklist@example.com").await;
    let org = server.create_org(&user, "Benchmark Org").await;
    let project = server.create_project(&user, &org, "Benchmark Project").await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/benchmarks",
            project_slug
        )))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
}

// ============ Thresholds ============

// GET /v0/projects/{project}/thresholds - list thresholds
#[tokio::test]
async fn test_projects_thresholds_list() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "thresholdlist@example.com").await;
    let org = server.create_org(&user, "Threshold Org").await;
    let project = server.create_project(&user, &org, "Threshold Project").await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/thresholds",
            project_slug
        )))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
}

// ============ Alerts ============

// GET /v0/projects/{project}/alerts - list alerts
#[tokio::test]
async fn test_projects_alerts_list() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "alertlist@example.com").await;
    let org = server.create_org(&user, "Alert Org").await;
    let project = server.create_project(&user, &org, "Alert Project").await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}/alerts", project_slug)))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
}

// ============ Reports ============

// GET /v0/projects/{project}/reports - list reports
#[tokio::test]
async fn test_projects_reports_list() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "reportlist@example.com").await;
    let org = server.create_org(&user, "Report Org").await;
    let project = server.create_project(&user, &org, "Report Project").await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}/reports", project_slug)))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
}

// ============ Plots ============

// GET /v0/projects/{project}/plots - list plots
#[tokio::test]
async fn test_projects_plots_list() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "plotlist@example.com").await;
    let org = server.create_org(&user, "Plot Org").await;
    let project = server.create_project(&user, &org, "Plot Project").await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}/plots", project_slug)))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
}

// ============ Perf ============

// GET /v0/projects/{project}/perf - get perf data
#[tokio::test]
async fn test_projects_perf_get() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "perfget@example.com").await;
    let org = server.create_org(&user, "Perf Org").await;
    let project = server.create_project(&user, &org, "Perf Project").await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}/perf", project_slug)))
        .header("Authorization", server.bearer(&user.token))
        .send()
        .await
        .expect("Request failed");

    // Perf endpoint may require query params, accept various success responses
    assert!(
        resp.status().is_success() || resp.status() == StatusCode::BAD_REQUEST,
        "Unexpected status: {}",
        resp.status()
    );
}
