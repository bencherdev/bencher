#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::missing_assert_message,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args,
    reason = "integration test file"
)]
//! Integration tests for project key authentication.

use bencher_api_tests::{TestServer, helpers::seed_token};
use bencher_json::{
    BranchUuid, JsonBenchmark, JsonBranch, JsonBranches, JsonMeasure, JsonPlot, JsonPlots,
    JsonProject, JsonProjectKey, JsonProjectKeyCreated, JsonProjectKeys, JsonReport, JsonTestbed,
    JsonThreshold, ProjectKey, ProjectKeyUuid,
};
use http::StatusCode;

async fn setup() -> (TestServer, String, String, ProjectKey) {
    let server = TestServer::new().await;
    let user = server.signup("Key User", "keyuser@example.com").await;
    let org = server.create_org(&user, "Key Org").await;
    let project = server.create_project(&user, &org, "Key Project").await;

    let project_slug: &str = project.slug.as_ref();
    let project_slug = project_slug.to_owned();

    // Create a project key
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/keys", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&serde_json::json!({"name": "ci-key"}))
        .send()
        .await
        .expect("Failed to create project key");

    assert_eq!(resp.status(), StatusCode::CREATED);
    let key_created: JsonProjectKeyCreated = resp.json().await.expect("Failed to parse key");

    (server, project_slug, user.token, key_created.key)
}

// GET /v0/projects/{project} - view project with key
#[tokio::test]
async fn project_key_get_project() {
    let (server, project_slug, _token, key) = setup().await;

    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let project: JsonProject = resp.json().await.expect("Failed to parse response");
    let slug: &str = project.slug.as_ref();
    assert_eq!(slug, project_slug);
}

// GET /v0/projects/{project}/branches - list branches with key
#[tokio::test]
async fn project_key_list_branches() {
    let (server, project_slug, _token, key) = setup().await;

    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}/branches", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let _branches: JsonBranches = resp.json().await.expect("Failed to parse response");
}

// GET /v0/projects/{project}/branches/{branch} - get single branch with key
// This exercises the PubProjectBearerToken extractor path (different from list endpoints)
#[tokio::test]
async fn project_key_get_branch() {
    let (server, project_slug, token, key) = setup().await;

    // Create a branch using JWT auth
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/branches", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&token),
        )
        .json(&serde_json::json!({"name": "feature-branch", "slug": "feature-branch"}))
        .send()
        .await
        .expect("Failed to create branch");
    assert_eq!(resp.status(), StatusCode::CREATED);

    // GET the branch using the project key
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/branches/feature-branch",
            project_slug
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let branch: JsonBranch = resp.json().await.expect("Failed to parse response");
    let name: &str = branch.name.as_ref();
    assert_eq!(name, "feature-branch");
}

// GET /v0/projects/{project}/testbeds - list testbeds with key
#[tokio::test]
async fn project_key_list_testbeds() {
    let (server, project_slug, _token, key) = setup().await;

    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}/testbeds", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
}

// GET /v0/projects/{project}/benchmarks - list benchmarks with key
#[tokio::test]
async fn project_key_list_benchmarks() {
    let (server, project_slug, _token, key) = setup().await;

    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}/benchmarks", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
}

// GET /v0/projects/{project}/measures - list measures with key
#[tokio::test]
async fn project_key_list_measures() {
    let (server, project_slug, _token, key) = setup().await;

    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}/measures", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
}

// GET /v0/projects/{project}/thresholds - list thresholds with key
#[tokio::test]
async fn project_key_list_thresholds() {
    let (server, project_slug, _token, key) = setup().await;

    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}/thresholds", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
}

// GET /v0/projects/{project}/alerts - list alerts with key
#[tokio::test]
async fn project_key_list_alerts() {
    let (server, project_slug, _token, key) = setup().await;

    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}/alerts", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
}

// GET /v0/projects/{project}/reports - list reports with key
#[tokio::test]
async fn project_key_list_reports() {
    let (server, project_slug, _token, key) = setup().await;

    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}/reports", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
}

// GET /v0/projects/{project}/plots - list plots with key
#[tokio::test]
async fn project_key_list_plots() {
    let (server, project_slug, _token, key) = setup().await;

    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}/plots", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn project_key_for_other_project_on_public_target_returns_403() {
    let server = TestServer::new().await;
    let user = server.signup("Key User", "keywrong@example.com").await;
    let org = server.create_org(&user, "Key Wrong Org").await;
    let project_a = server.create_project(&user, &org, "Project A").await;
    let project_b = server.create_project(&user, &org, "Project B").await;

    let slug_a: &str = project_a.slug.as_ref();
    let slug_b: &str = project_b.slug.as_ref();

    // Create key for project A
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/keys", slug_a)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&serde_json::json!({"name": "a-key"}))
        .send()
        .await
        .expect("Failed to create key");

    let key_created: JsonProjectKeyCreated = resp.json().await.expect("Failed to parse key");

    // Try to access project B with project A's key
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}/branches", slug_b)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key_created.key.as_ref()),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let body = resp.text().await.expect("Failed to read response body");
    assert!(
        body.contains("access denied"),
        "Expected 'access denied' in body, got: {}",
        body
    );
}

#[cfg(feature = "plus")]
#[tokio::test]
async fn project_key_for_other_project_on_private_target_returns_404() {
    use bencher_json::project::Visibility;
    use bencher_schema::schema;
    use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};

    let server = TestServer::new().await;
    let user = server.signup("Key User", "keywrongpriv@example.com").await;
    let org = server.create_org(&user, "Key Wrong Priv Org").await;
    let project_a = server.create_project(&user, &org, "Priv Project A").await;
    let project_b = server.create_project(&user, &org, "Priv Project B").await;

    {
        let mut conn = server.db_conn();
        diesel::update(schema::project::table.filter(schema::project::uuid.eq(project_b.uuid)))
            .set(schema::project::visibility.eq(Visibility::Private))
            .execute(&mut conn)
            .expect("Failed to update project visibility");
    }

    let slug_a: &str = project_a.slug.as_ref();
    let slug_b: &str = project_b.slug.as_ref();

    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/keys", slug_a)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&serde_json::json!({"name": "a-key"}))
        .send()
        .await
        .expect("Failed to create key");
    let key_created: JsonProjectKeyCreated = resp.json().await.expect("Failed to parse key");

    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}/branches", slug_b)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key_created.key.as_ref()),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body = resp.text().await.expect("Failed to read response body");
    assert!(
        body.contains("may be private"),
        "Expected info-hiding wording in body, got: {}",
        body
    );
}

// Negative: project key cannot list keys (requires Manage permission)
#[tokio::test]
async fn project_key_cannot_list_keys() {
    let (server, project_slug, _token, key) = setup().await;

    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}/keys", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// POST /v0/projects/{project}/branches - create branch with key
#[tokio::test]
async fn project_key_can_create_branch() {
    let (server, project_slug, _token, key) = setup().await;

    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/branches", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .json(&serde_json::json!({"name": "key-branch"}))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
    let branch: JsonBranch = resp.json().await.expect("Failed to parse response");
    let name: &str = branch.name.as_ref();
    assert_eq!(name, "key-branch");
}

// POST /v0/projects/{project}/testbeds - create testbed with key
#[tokio::test]
async fn project_key_can_create_testbed() {
    let (server, project_slug, _token, key) = setup().await;

    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/testbeds", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .json(&serde_json::json!({"name": "key-testbed"}))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
    let testbed: JsonTestbed = resp.json().await.expect("Failed to parse response");
    let name: &str = testbed.name.as_ref();
    assert_eq!(name, "key-testbed");
}

// POST /v0/projects/{project}/benchmarks - create benchmark with key
#[tokio::test]
async fn project_key_can_create_benchmark() {
    let (server, project_slug, _token, key) = setup().await;

    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/benchmarks", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .json(&serde_json::json!({"name": "key-benchmark"}))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
    let benchmark: JsonBenchmark = resp.json().await.expect("Failed to parse response");
    let name: &str = benchmark.name.as_ref();
    assert_eq!(name, "key-benchmark");
}

// POST /v0/projects/{project}/measures - create measure with key
#[tokio::test]
async fn project_key_can_create_measure() {
    let (server, project_slug, _token, key) = setup().await;

    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/measures", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .json(&serde_json::json!({"name": "key-measure", "units": "ns/iter"}))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
    let measure: JsonMeasure = resp.json().await.expect("Failed to parse response");
    let name: &str = measure.name.as_ref();
    assert_eq!(name, "key-measure");
}

// POST /v0/projects/{project}/thresholds - create threshold with key
#[tokio::test]
async fn project_key_can_create_threshold() {
    let (server, project_slug, token, key) = setup().await;

    // Create branch, testbed, and measure using JWT auth
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/branches", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&token),
        )
        .json(&serde_json::json!({"name": "threshold-branch"}))
        .send()
        .await
        .expect("Failed to create branch");
    assert_eq!(resp.status(), StatusCode::CREATED);

    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/testbeds", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&token),
        )
        .json(&serde_json::json!({"name": "threshold-testbed"}))
        .send()
        .await
        .expect("Failed to create testbed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/measures", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&token),
        )
        .json(&serde_json::json!({"name": "threshold-measure", "units": "ns/iter"}))
        .send()
        .await
        .expect("Failed to create measure");
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Create threshold using project key
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/thresholds", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .json(&serde_json::json!({
            "branch": "threshold-branch",
            "testbed": "threshold-testbed",
            "measure": "threshold-measure",
            "test": "percentage",
            "upper_boundary": 0.25
        }))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
    let _threshold: JsonThreshold = resp.json().await.expect("Failed to parse response");
}

// POST /v0/projects/{project}/reports - project key CAN create report
#[tokio::test]
async fn project_key_can_create_report() {
    let (server, project_slug, _token, key) = setup().await;

    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/reports", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .json(&serde_json::json!({
            "branch": "main",
            "testbed": "localhost",
            "start_time": "2024-01-01T00:00:00Z",
            "end_time": "2024-01-01T00:01:00Z",
            "results": ["{\"bench_name\": {\"latency\": {\"value\": 100.0}}}"]
        }))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
    let _report: JsonReport = resp.json().await.expect("Failed to parse response");
}

#[tokio::test]
async fn project_key_cannot_create_in_wrong_project() {
    let server = TestServer::new().await;
    let user = server
        .signup("Key User", "keycrosscreate@example.com")
        .await;
    let org = server.create_org(&user, "Cross Create Org").await;
    let project_a = server.create_project(&user, &org, "Create A").await;
    let project_b = server.create_project(&user, &org, "Create B").await;

    let slug_a: &str = project_a.slug.as_ref();
    let slug_b: &str = project_b.slug.as_ref();

    // Create key for project A
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/keys", slug_a)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&serde_json::json!({"name": "cross-key"}))
        .send()
        .await
        .expect("Failed to create key");
    let key_created: JsonProjectKeyCreated = resp.json().await.expect("Failed to parse key");

    // Try to create a branch in project B with project A's key
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/branches", slug_b)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key_created.key.as_ref()),
        )
        .json(&serde_json::json!({"name": "cross-branch"}))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let body = resp.text().await.expect("Failed to read response body");
    assert!(
        body.contains("access denied"),
        "Expected 'access denied' in body, got: {}",
        body
    );
}

#[cfg(feature = "plus")]
#[tokio::test]
async fn project_key_cannot_create_in_wrong_private_project_returns_404() {
    use bencher_json::project::Visibility;
    use bencher_schema::schema;
    use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};

    let server = TestServer::new().await;
    let user = server
        .signup("Key User", "keycrosscreatepriv@example.com")
        .await;
    let org = server.create_org(&user, "Cross Create Priv Org").await;
    let project_a = server.create_project(&user, &org, "Priv Create A").await;
    let project_b = server.create_project(&user, &org, "Priv Create B").await;

    {
        let mut conn = server.db_conn();
        diesel::update(schema::project::table.filter(schema::project::uuid.eq(project_b.uuid)))
            .set(schema::project::visibility.eq(Visibility::Private))
            .execute(&mut conn)
            .expect("Failed to update project visibility");
    }

    let slug_a: &str = project_a.slug.as_ref();
    let slug_b: &str = project_b.slug.as_ref();

    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/keys", slug_a)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&serde_json::json!({"name": "cross-priv-key"}))
        .send()
        .await
        .expect("Failed to create key");
    let key_created: JsonProjectKeyCreated = resp.json().await.expect("Failed to parse key");

    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/branches", slug_b)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key_created.key.as_ref()),
        )
        .json(&serde_json::json!({"name": "cross-priv-branch"}))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body = resp.text().await.expect("Failed to read response body");
    assert!(
        body.contains("may be private"),
        "Expected info-hiding wording in body, got: {}",
        body
    );
}

// Negative: revoked project key is rejected
#[tokio::test]
async fn project_key_revoked() {
    let server = TestServer::new().await;
    let user = server.signup("Key User", "keyrevoked@example.com").await;
    let org = server.create_org(&user, "Revoke Key Org").await;
    let project = server.create_project(&user, &org, "Revoke Project").await;

    let project_slug: &str = project.slug.as_ref();

    // Create a project key
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/keys", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&serde_json::json!({"name": "revoke-key"}))
        .send()
        .await
        .expect("Failed to create key");

    assert_eq!(resp.status(), StatusCode::CREATED);
    let key_created: JsonProjectKeyCreated = resp.json().await.expect("Failed to parse key");

    // Verify key works
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key_created.key.as_ref()),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);

    // Revoke the key
    let resp = server
        .client
        .delete(server.api_url(&format!(
            "/v0/projects/{}/keys/{}",
            project_slug, key_created.uuid
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Failed to revoke key");
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Revoked key should be rejected
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key_created.key.as_ref()),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// PATCH /v0/projects/{project}/branches/{branch} - project key cannot rename
#[tokio::test]
async fn project_key_cannot_rename_branch() {
    let (server, project_slug, token, key) = setup().await;

    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/branches", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&token),
        )
        .json(&serde_json::json!({"name": "rename-branch"}))
        .send()
        .await
        .expect("Failed to create branch");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let branch: JsonBranch = resp.json().await.expect("Failed to parse branch");

    let resp = server
        .client
        .patch(server.api_url(&format!(
            "/v0/projects/{}/branches/{}",
            project_slug, branch.slug
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .json(&serde_json::json!({"name": "new-name"}))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// PATCH /v0/projects/{project}/branches/{branch} - project key CAN archive
#[tokio::test]
async fn project_key_can_archive_branch() {
    let (server, project_slug, token, key) = setup().await;

    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/branches", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&token),
        )
        .json(&serde_json::json!({"name": "archive-branch"}))
        .send()
        .await
        .expect("Failed to create branch");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let branch: JsonBranch = resp.json().await.expect("Failed to parse branch");

    let resp = server
        .client
        .patch(server.api_url(&format!(
            "/v0/projects/{}/branches/{}",
            project_slug, branch.slug
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .json(&serde_json::json!({"archived": true}))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
}

// PATCH /v0/projects/{project}/benchmarks/{benchmark} - project key cannot rename
#[tokio::test]
async fn project_key_cannot_rename_benchmark() {
    let (server, project_slug, token, key) = setup().await;

    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/benchmarks", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&token),
        )
        .json(&serde_json::json!({"name": "rename-benchmark"}))
        .send()
        .await
        .expect("Failed to create benchmark");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let benchmark: JsonBenchmark = resp.json().await.expect("Failed to parse benchmark");

    let resp = server
        .client
        .patch(server.api_url(&format!(
            "/v0/projects/{}/benchmarks/{}",
            project_slug, benchmark.slug
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .json(&serde_json::json!({"name": "new-name"}))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// PATCH /v0/projects/{project}/testbeds/{testbed} - project key cannot rename
#[tokio::test]
async fn project_key_cannot_rename_testbed() {
    let (server, project_slug, token, key) = setup().await;

    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/testbeds", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&token),
        )
        .json(&serde_json::json!({"name": "rename-testbed"}))
        .send()
        .await
        .expect("Failed to create testbed");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let testbed: JsonTestbed = resp.json().await.expect("Failed to parse testbed");

    let resp = server
        .client
        .patch(server.api_url(&format!(
            "/v0/projects/{}/testbeds/{}",
            project_slug, testbed.slug
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .json(&serde_json::json!({"name": "new-name"}))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// PATCH /v0/projects/{project}/measures/{measure} - project key cannot rename
#[tokio::test]
async fn project_key_cannot_rename_measure() {
    let (server, project_slug, token, key) = setup().await;

    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/measures", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&token),
        )
        .json(&serde_json::json!({"name": "rename-measure", "units": "ns/iter"}))
        .send()
        .await
        .expect("Failed to create measure");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let measure: JsonMeasure = resp.json().await.expect("Failed to parse measure");

    let resp = server
        .client
        .patch(server.api_url(&format!(
            "/v0/projects/{}/measures/{}",
            project_slug, measure.slug
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .json(&serde_json::json!({"name": "new-name"}))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// PUT /v0/projects/{project}/thresholds/{threshold} - project key CAN update model
#[tokio::test]
async fn project_key_can_update_threshold() {
    let (server, project_slug, token, key) = setup().await;

    // Create branch, testbed, measure, and threshold with user token
    server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/branches", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&token),
        )
        .json(&serde_json::json!({"name": "thresh-branch"}))
        .send()
        .await
        .expect("Failed to create branch");

    server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/testbeds", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&token),
        )
        .json(&serde_json::json!({"name": "thresh-testbed"}))
        .send()
        .await
        .expect("Failed to create testbed");

    server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/measures", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&token),
        )
        .json(&serde_json::json!({"name": "thresh-measure", "units": "ns/iter"}))
        .send()
        .await
        .expect("Failed to create measure");

    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/thresholds", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&token),
        )
        .json(&serde_json::json!({
            "branch": "thresh-branch",
            "testbed": "thresh-testbed",
            "measure": "thresh-measure",
            "test": "percentage",
            "upper_boundary": 0.25
        }))
        .send()
        .await
        .expect("Failed to create threshold");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let threshold: JsonThreshold = resp.json().await.expect("Failed to parse threshold");

    // PUT with project key to update model
    let resp = server
        .client
        .put(server.api_url(&format!(
            "/v0/projects/{}/thresholds/{}",
            project_slug, threshold.uuid
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .json(&serde_json::json!({
            "test": "percentage",
            "upper_boundary": 0.50
        }))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
}

// PATCH /v0/projects/{project}/alerts/{alert} - project key CAN dismiss alert
#[tokio::test]
async fn project_key_can_dismiss_alert() {
    let (server, project_slug, _token, key) = setup().await;

    // Submit a run with a tight threshold to generate an alert.
    // First, submit a baseline run to establish metrics.
    server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/reports", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .json(&serde_json::json!({
            "branch": "main",
            "testbed": "localhost",
            "thresholds": {
                "models": {
                    "latency": {
                        "test": "percentage",
                        "min_sample_size": 2,
                        "upper_boundary": 0.01
                    }
                }
            },
            "start_time": "2024-01-01T00:00:00Z",
            "end_time": "2024-01-01T00:01:00Z",
            "results": ["{\"bench\": {\"latency\": {\"value\": 100.0}}}"]
        }))
        .send()
        .await
        .expect("Failed to submit baseline");

    // Submit a second run with a much higher value to trigger an alert
    server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/reports", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .json(&serde_json::json!({
            "branch": "main",
            "testbed": "localhost",
            "start_time": "2024-01-01T00:02:00Z",
            "end_time": "2024-01-01T00:03:00Z",
            "results": ["{\"bench\": {\"latency\": {\"value\": 100.0}}}"]
        }))
        .send()
        .await
        .expect("Failed to submit second run");

    // Submit a third run with a spike to trigger an alert (need min_sample_size of 2)
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/reports", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .json(&serde_json::json!({
            "branch": "main",
            "testbed": "localhost",
            "start_time": "2024-01-01T00:04:00Z",
            "end_time": "2024-01-01T00:05:00Z",
            "results": ["{\"bench\": {\"latency\": {\"value\": 999999.0}}}"]
        }))
        .send()
        .await
        .expect("Failed to submit spike run");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let report: JsonReport = resp.json().await.expect("Failed to parse report");

    // Check that an alert was generated
    let alerts = report.alerts.as_deref().expect("Report missing alerts");
    assert!(
        !alerts.is_empty(),
        "Expected at least one alert from the spike run"
    );

    let alert_uuid = alerts[0].uuid;

    // Dismiss the alert with the project key
    let resp = server
        .client
        .patch(server.api_url(&format!(
            "/v0/projects/{}/alerts/{}",
            project_slug, alert_uuid
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .json(&serde_json::json!({"status": "dismissed"}))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
}

// POST /v0/projects/{project}/plots - create plot with key
#[tokio::test]
async fn project_key_can_create_plot() {
    let (server, project_slug, _token, key) = setup().await;
    let plot = new_plot(&server, &project_slug, key.as_ref()).await;

    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/plots", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .json(&plot)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
    let plot: JsonPlot = resp.json().await.expect("Failed to parse response");
    let title: Option<&str> = plot.title.as_ref().map(AsRef::as_ref);
    assert_eq!(title, Some("Key Plot"));
}

// PATCH /v0/projects/{project}/plots/{plot} - project key CAN update, title included
#[tokio::test]
async fn project_key_can_update_plot() {
    let (server, project_slug, token, key) = setup().await;
    let plot = new_plot(&server, &project_slug, &token).await;

    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/plots", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&token),
        )
        .json(&plot)
        .send()
        .await
        .expect("Failed to create plot");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let plot: JsonPlot = resp.json().await.expect("Failed to parse plot");

    let resp = server
        .client
        .patch(server.api_url(&format!(
            "/v0/projects/{}/plots/{}",
            project_slug, plot.uuid
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .json(&serde_json::json!({"title": "Retitled", "upper_boundary": false}))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let plot: JsonPlot = resp.json().await.expect("Failed to parse response");
    let title: Option<&str> = plot.title.as_ref().map(AsRef::as_ref);
    assert_eq!(title, Some("Retitled"));
    assert!(!plot.upper_boundary);
}

#[tokio::test]
async fn project_key_cannot_create_or_update_plot_in_wrong_project() {
    let server = TestServer::new().await;
    let user = server.signup("Key User", "keycrossplot@example.com").await;
    let org = server.create_org(&user, "Cross Plot Org").await;
    let project_a = server.create_project(&user, &org, "Plot A").await;
    let project_b = server.create_project(&user, &org, "Plot B").await;

    let slug_a: &str = project_a.slug.as_ref();
    let slug_b: &str = project_b.slug.as_ref();
    let key_a = server
        .create_project_key(&user, &project_a, "cross-plot-key")
        .await;

    let plot = new_plot(&server, slug_b, &user.token).await;
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/plots", slug_b)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&plot)
        .send()
        .await
        .expect("Failed to create plot");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let plot_b: JsonPlot = resp.json().await.expect("Failed to parse plot");

    // Try to create a plot in project B with project A's key
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/plots", slug_b)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key_a.key.as_ref()),
        )
        .json(&plot)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let body = resp.text().await.expect("Failed to read response body");
    assert!(
        body.contains("access denied"),
        "Expected 'access denied' in body, got: {}",
        body
    );

    // Try to update project B's plot with project A's key
    let resp = server
        .client
        .patch(server.api_url(&format!("/v0/projects/{}/plots/{}", slug_b, plot_b.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key_a.key.as_ref()),
        )
        .json(&serde_json::json!({"title": "Cross"}))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let body = resp.text().await.expect("Failed to read response body");
    assert!(
        body.contains("access denied"),
        "Expected 'access denied' in body, got: {}",
        body
    );

    // Nor can project A's key reach project B's plot through its own project
    let resp = server
        .client
        .patch(server.api_url(&format!("/v0/projects/{}/plots/{}", slug_a, plot_b.uuid)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key_a.key.as_ref()),
        )
        .json(&serde_json::json!({"title": "Cross"}))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// DELETE /v0/projects/{project}/plots/{plot} - project key cannot delete
#[tokio::test]
async fn project_key_cannot_delete_plot() {
    let (server, project_slug, token, key) = setup().await;
    let plot = new_plot(&server, &project_slug, &token).await;

    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/plots", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&token),
        )
        .json(&plot)
        .send()
        .await
        .expect("Failed to create plot");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let plot: JsonPlot = resp.json().await.expect("Failed to parse plot");
    let plot_url = server.api_url(&format!(
        "/v0/projects/{}/plots/{}",
        project_slug, plot.uuid
    ));

    let resp = server
        .client
        .delete(&plot_url)
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // The plot is not deleted.
    let resp = server
        .client
        .get(&plot_url)
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key.as_ref()),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
}

// POST /v0/projects/{project}/plots - a branch from another project is not found
#[tokio::test]
async fn project_key_cannot_create_plot_with_other_project_branch() {
    let ForeignBranch {
        server,
        slug_a,
        key_a,
        mut plot_a,
        branch_b,
    } = setup_foreign_branch().await;
    plot_a["branches"] = serde_json::json!([branch_b]);
    let plots_url = server.api_url(&format!("/v0/projects/{}/plots", slug_a));

    let resp = server
        .client
        .post(&plots_url)
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key_a.as_ref()),
        )
        .json(&plot_a)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body = resp.text().await.expect("Failed to read response body");
    assert!(
        body.contains(&branch_b.to_string()),
        "Expected the foreign branch in body, got: {}",
        body
    );

    // No plot is created.
    let resp = server
        .client
        .get(&plots_url)
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key_a.as_ref()),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let plots: JsonPlots = resp.json().await.expect("Failed to parse response");
    assert!(plots.0.is_empty());
}

// PATCH /v0/projects/{project}/plots/{plot} - a branch from another project is not found
#[tokio::test]
async fn project_key_cannot_update_plot_with_other_project_branch() {
    let ForeignBranch {
        server,
        slug_a,
        key_a,
        plot_a,
        branch_b,
    } = setup_foreign_branch().await;

    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/plots", slug_a)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key_a.as_ref()),
        )
        .json(&plot_a)
        .send()
        .await
        .expect("Failed to create plot");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let created: serde_json::Value = resp.json().await.expect("Failed to parse plot");
    let plot_url = server.api_url(&format!(
        "/v0/projects/{}/plots/{}",
        slug_a,
        created["uuid"].as_str().expect("Plot UUID")
    ));

    let resp = server
        .client
        .patch(&plot_url)
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key_a.as_ref()),
        )
        .json(&serde_json::json!({"title": "Foreign", "branches": [branch_b]}))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body = resp.text().await.expect("Failed to read response body");
    assert!(
        body.contains(&branch_b.to_string()),
        "Expected the foreign branch in body, got: {}",
        body
    );

    // The plot is unchanged.
    let resp = server
        .client
        .get(&plot_url)
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(key_a.as_ref()),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let plot: serde_json::Value = resp.json().await.expect("Failed to parse response");
    assert_eq!(plot, created);
}

/// A key and a new plot body for project A, and a branch in project B.
struct ForeignBranch {
    server: TestServer,
    slug_a: String,
    key_a: ProjectKey,
    plot_a: serde_json::Value,
    branch_b: BranchUuid,
}

async fn setup_foreign_branch() -> ForeignBranch {
    let server = TestServer::new().await;
    let user = server.signup("Key User", "keyuser@example.com").await;
    let org = server.create_org(&user, "Foreign Plot Org").await;
    let project_a = server.create_project(&user, &org, "Foreign A").await;
    let project_b = server.create_project(&user, &org, "Foreign B").await;
    let slug_a: &str = project_a.slug.as_ref();
    let slug_b: &str = project_b.slug.as_ref();
    let key_a = server
        .create_project_key(&user, &project_a, "foreign-plot-key")
        .await;
    let plot_a = new_plot(&server, slug_a, key_a.key.as_ref()).await;

    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/branches", slug_b)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&serde_json::json!({"name": "foreign-branch"}))
        .send()
        .await
        .expect("Failed to create branch");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let branch_b: JsonBranch = resp.json().await.expect("Failed to parse branch");

    ForeignBranch {
        server,
        slug_a: slug_a.to_owned(),
        key_a: key_a.key,
        plot_a,
        branch_b: branch_b.uuid,
    }
}

/// A new plot body for a project, with a branch, testbed, benchmark, and measure created by `bearer`.
async fn new_plot(server: &TestServer, project_slug: &str, bearer: &str) -> serde_json::Value {
    let create = async |resource: &str, body: serde_json::Value| {
        let resp = server
            .client
            .post(server.api_url(&format!("/v0/projects/{}/{}", project_slug, resource)))
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(bearer),
            )
            .json(&body)
            .send()
            .await
            .expect("Request failed");
        assert_eq!(resp.status(), StatusCode::CREATED, "{}", resource);
        resp
    };
    let branch: JsonBranch = create("branches", serde_json::json!({"name": "plot-branch"}))
        .await
        .json()
        .await
        .expect("Failed to parse branch");
    let testbed: JsonTestbed = create("testbeds", serde_json::json!({"name": "plot-testbed"}))
        .await
        .json()
        .await
        .expect("Failed to parse testbed");
    let benchmark: JsonBenchmark =
        create("benchmarks", serde_json::json!({"name": "plot-benchmark"}))
            .await
            .json()
            .await
            .expect("Failed to parse benchmark");
    let measure: JsonMeasure = create(
        "measures",
        serde_json::json!({"name": "plot-measure", "units": "ns"}),
    )
    .await
    .json()
    .await
    .expect("Failed to parse measure");

    serde_json::json!({
        "title": "Key Plot",
        "lower_value": false,
        "upper_value": false,
        "lower_boundary": false,
        "upper_boundary": true,
        "x_axis": "date_time",
        "window": 2_592_000,
        "branches": [branch.uuid],
        "testbeds": [testbed.uuid],
        "benchmarks": [benchmark.uuid],
        "measures": [measure.uuid],
    })
}

// Fails if project key POST accepts a deprecated user API token or a user API key.
#[tokio::test]
async fn project_key_create_requires_session() {
    let server = TestServer::new().await;
    let user = server.signup("Key User", "keysession@example.com").await;
    let org = server.create_org(&user, "Session Key Org").await;
    let project = server
        .create_project(&user, &org, "Session Key Project")
        .await;
    let project_slug: &str = project.slug.as_ref();
    let api_token = seed_token(&server, &user, "legacy-token");
    let user_key = server.create_user_key(&user, "user-key").await;

    let status = create_key(&server, project_slug, api_token.token.as_ref()).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let status = create_key(&server, project_slug, user_key.as_ref()).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let status = create_key(&server, project_slug, &user.token).await;
    assert_eq!(status, StatusCode::CREATED);

    // Only the session created a key.
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}/keys", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let keys: JsonProjectKeys = resp.json().await.expect("Failed to parse keys");
    assert_eq!(keys.0.len(), 1);
}

async fn create_key(server: &TestServer, project_slug: &str, bearer: &str) -> StatusCode {
    server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/keys", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(bearer),
        )
        .json(&serde_json::json!({"name": "session-key"}))
        .send()
        .await
        .expect("Request failed")
        .status()
}

// Fails if project key list accepts a user API key or a deprecated user API token.
#[tokio::test]
async fn project_key_list_requires_session() {
    let fixture = setup_non_session().await;

    for bearer in &fixture.bearers {
        let resp = fixture
            .server
            .client
            .get(
                fixture
                    .server
                    .api_url(&format!("/v0/projects/{}/keys", fixture.project_slug)),
            )
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(bearer),
            )
            .send()
            .await
            .expect("Request failed");
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }
}

// Fails if project key view accepts a user API key or a deprecated user API token.
#[tokio::test]
async fn project_key_view_requires_session() {
    let fixture = setup_non_session().await;

    for bearer in &fixture.bearers {
        let resp = fixture
            .server
            .client
            .get(fixture.key_url())
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(bearer),
            )
            .send()
            .await
            .expect("Request failed");
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }
}

// Fails if project key PATCH accepts a user API key or a deprecated user API token, or refuses a user session.
#[tokio::test]
async fn project_key_rename_requires_session() {
    let fixture = setup_non_session().await;

    for bearer in &fixture.bearers {
        let resp = fixture
            .server
            .client
            .patch(fixture.key_url())
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(bearer),
            )
            .json(&serde_json::json!({"name": "tampered"}))
            .send()
            .await
            .expect("Request failed");
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    // The key's name is unchanged.
    let resp = fixture
        .server
        .client
        .get(fixture.key_url())
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&fixture.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let viewed: JsonProjectKey = resp.json().await.expect("Failed to parse key");
    let name: &str = viewed.name.as_ref();
    assert_eq!(name, "ci-key");

    let resp = fixture
        .server
        .client
        .patch(fixture.key_url())
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&fixture.token),
        )
        .json(&serde_json::json!({"name": "renamed"}))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let renamed: JsonProjectKey = resp.json().await.expect("Failed to parse key");
    let name: &str = renamed.name.as_ref();
    assert_eq!(name, "renamed");
}

// Fails if project key DELETE accepts a user API key or a deprecated user API token.
#[tokio::test]
async fn project_key_revoke_requires_session() {
    let fixture = setup_non_session().await;

    for bearer in &fixture.bearers {
        let resp = fixture
            .server
            .client
            .delete(fixture.key_url())
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(bearer),
            )
            .send()
            .await
            .expect("Request failed");
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    // The key is not revoked and still authenticates.
    let resp = fixture
        .server
        .client
        .get(
            fixture
                .server
                .api_url(&format!("/v0/projects/{}", fixture.project_slug)),
        )
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(fixture.key.as_ref()),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
}

struct NonSession {
    server: TestServer,
    project_slug: String,
    token: String,
    key: ProjectKey,
    key_uuid: ProjectKeyUuid,
    /// A user API key and a deprecated user API token for the project's manager.
    bearers: [String; 2],
}

impl NonSession {
    fn key_url(&self) -> String {
        self.server.api_url(&format!(
            "/v0/projects/{}/keys/{}",
            self.project_slug, self.key_uuid
        ))
    }
}

async fn setup_non_session() -> NonSession {
    let server = TestServer::new().await;
    let user = server.signup("Key User", "keyuser@example.com").await;
    let org = server.create_org(&user, "Key Org").await;
    let project = server.create_project(&user, &org, "Key Project").await;
    let project_slug: &str = project.slug.as_ref();
    let project_slug = project_slug.to_owned();

    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/keys", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&serde_json::json!({"name": "ci-key"}))
        .send()
        .await
        .expect("Failed to create project key");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let created: JsonProjectKeyCreated = resp.json().await.expect("Failed to parse key");

    let user_key = server.create_user_key(&user, "user-key").await;
    let api_token = seed_token(&server, &user, "legacy-token");
    NonSession {
        server,
        project_slug,
        token: user.token,
        key: created.key,
        key_uuid: created.uuid,
        bearers: [
            user_key.as_ref().to_owned(),
            api_token.token.as_ref().to_owned(),
        ],
    }
}

// GET /v0/projects/{project}/keys?sort=created - oldest first, a page at a time
#[tokio::test]
async fn project_keys_sort_created() {
    let server = TestServer::new().await;
    let user = server.signup("Sort User", "keysort@example.com").await;
    let org = server.create_org(&user, "Sort Org").await;
    let project = server.create_project(&user, &org, "Sort Project").await;
    let project_slug: &str = project.slug.as_ref();
    for name in ["charlie", "alpha", "bravo"] {
        let resp = server
            .client
            .post(server.api_url(&format!("/v0/projects/{}/keys", project_slug)))
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(&user.token),
            )
            .json(&serde_json::json!({ "name": name }))
            .send()
            .await
            .expect("Failed to create project key");
        assert_eq!(resp.status(), StatusCode::CREATED);
    }

    let names = |query: &'static str| {
        let server = &server;
        let token = &user.token;
        async move {
            let resp = server
                .client
                .get(server.api_url(&format!("/v0/projects/{}/keys?{}", project_slug, query)))
                .header(
                    bencher_json::AUTHORIZATION,
                    bencher_json::bearer_header(token),
                )
                .send()
                .await
                .expect("Request failed");
            assert_eq!(resp.status(), StatusCode::OK);
            let keys: JsonProjectKeys = resp.json().await.expect("Failed to parse keys");
            keys.0
                .into_iter()
                .map(|key| key.name.to_string())
                .collect::<Vec<_>>()
        }
    };

    assert_eq!(names("per_page=2").await, ["alpha", "bravo"]);
    assert_eq!(names("sort=created&per_page=2").await, ["charlie", "alpha"]);
    assert_eq!(names("sort=created&per_page=2&page=2").await, ["bravo"]);
    assert_eq!(
        names("sort=created&direction=desc").await,
        ["bravo", "alpha", "charlie"]
    );
}
