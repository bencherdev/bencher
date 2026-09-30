#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::missing_assert_message,
    clippy::similar_names,
    clippy::tests_outside_test_module,
    clippy::too_many_lines,
    clippy::uninlined_format_args,
    reason = "integration test file"
)]
//! Integration tests for project threshold endpoints.

#[cfg(feature = "plus")]
use bencher_api_tests::helpers::get_project_id;
use bencher_api_tests::{TestServer, TestUser};
#[cfg(feature = "plus")]
use bencher_json::ThresholdUuid;
use bencher_json::{JsonThreshold, JsonThresholds, ModelUuid};
#[cfg(feature = "plus")]
use bencher_schema::{
    model::project::{
        ProjectId,
        threshold::{InsertThreshold, QueryThreshold},
    },
    schema,
};
#[cfg(feature = "plus")]
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use http::StatusCode;

// GET /v0/projects/{project}/thresholds - list thresholds
#[tokio::test]
async fn thresholds_list() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "thresholdlist@example.com")
        .await;
    let org = server.create_org(&user, "Threshold Org").await;
    let project = server
        .create_project(&user, &org, "Threshold Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}/thresholds", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let thresholds: JsonThresholds = resp.json().await.expect("Failed to parse response");
    // New project should have no thresholds
    assert!(thresholds.0.is_empty());
}

// GET /v0/projects/{project}/thresholds - requires auth
#[tokio::test]
async fn thresholds_list_requires_auth() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "thresholdauth@example.com")
        .await;
    let org = server.create_org(&user, "Threshold Auth Org").await;
    let project = server
        .create_project(&user, &org, "Threshold Auth Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}/thresholds", project_slug)))
        .send()
        .await
        .expect("Request failed");

    // Public project can be viewed without auth
    assert!(
        resp.status() == StatusCode::OK || resp.status() == StatusCode::BAD_REQUEST,
        "Expected OK or BAD_REQUEST, got: {}",
        resp.status()
    );
}

// GET /v0/projects/{project}/thresholds/{threshold} - not found
#[tokio::test]
async fn thresholds_get_not_found() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "thresholdnotfound@example.com")
        .await;
    let org = server.create_org(&user, "Threshold NotFound Org").await;
    let project = server
        .create_project(&user, &org, "Threshold NotFound Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/thresholds/00000000-0000-0000-0000-000000000000",
            project_slug
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

/// Helper: create a branch, testbed, measure, and threshold for a project.
/// Returns the created `JsonThreshold`.
async fn create_threshold_with_model(
    server: &TestServer,
    user: &TestUser,
    project_slug: &str,
) -> JsonThreshold {
    // Create a branch
    let body = serde_json::json!({
        "name": "threshold-branch",
        "slug": "threshold-branch"
    });
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/branches", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Create a testbed
    let body = serde_json::json!({
        "name": "threshold-testbed",
        "slug": "threshold-testbed"
    });
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/testbeds", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Create a measure
    let body = serde_json::json!({
        "name": "throughput",
        "slug": "throughput",
        "units": "ops/sec"
    });
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/measures", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Create a threshold with a t-test model
    let body = serde_json::json!({
        "branch": "threshold-branch",
        "testbed": "threshold-testbed",
        "measure": "throughput",
        "test": "t_test",
        "max_sample_size": 64,
        "upper_boundary": 0.99
    });
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/thresholds", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);
    resp.json().await.expect("Failed to parse threshold")
}

// GET /v0/projects/{project}/thresholds/{threshold}?model= - view threshold with specific model
#[tokio::test]
async fn thresholds_get_with_model_query() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "thresholdmodel@example.com")
        .await;
    let org = server.create_org(&user, "Threshold Model Org").await;
    let project = server
        .create_project(&user, &org, "Threshold Model Project")
        .await;
    let project_slug: &str = project.slug.as_ref();

    // Create threshold with initial model (model A)
    let threshold = create_threshold_with_model(&server, &user, project_slug).await;
    let model_a = threshold.model.expect("threshold should have a model");
    let model_a_uuid = model_a.uuid;

    // GET without ?model= returns current model (model A)
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/thresholds/{}",
            project_slug, threshold.uuid
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let t: JsonThreshold = resp.json().await.expect("Failed to parse threshold");
    assert_eq!(
        t.model.as_ref().expect("model should exist").uuid,
        model_a_uuid
    );

    // PUT to update threshold with a new model (model B)
    let update_body = serde_json::json!({
        "test": "percentage",
        "max_sample_size": 32,
        "upper_boundary": 0.05
    });
    let resp = server
        .client
        .put(server.api_url(&format!(
            "/v0/projects/{}/thresholds/{}",
            project_slug, threshold.uuid
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&update_body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let updated: JsonThreshold = resp
        .json()
        .await
        .expect("Failed to parse updated threshold");
    let model_b = updated
        .model
        .expect("updated threshold should have a model");
    let model_b_uuid = model_b.uuid;
    // New model should be different from old model
    assert_ne!(model_a_uuid, model_b_uuid);

    // GET without ?model= returns current model (model B)
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/thresholds/{}",
            project_slug, threshold.uuid
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let t: JsonThreshold = resp.json().await.expect("Failed to parse threshold");
    assert_eq!(
        t.model.as_ref().expect("model should exist").uuid,
        model_b_uuid
    );

    // GET with ?model=<model_a_uuid> returns historical model A
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/thresholds/{}?model={}",
            project_slug, threshold.uuid, model_a_uuid
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let t: JsonThreshold = resp.json().await.expect("Failed to parse threshold");
    assert_eq!(
        t.model.as_ref().expect("model should exist").uuid,
        model_a_uuid
    );

    // GET with ?model=<model_b_uuid> returns current model B explicitly
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/thresholds/{}?model={}",
            project_slug, threshold.uuid, model_b_uuid
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let t: JsonThreshold = resp.json().await.expect("Failed to parse threshold");
    assert_eq!(
        t.model.as_ref().expect("model should exist").uuid,
        model_b_uuid
    );
}

// GET /v0/projects/{project}/thresholds/{threshold}?model=<nonexistent> - returns 404
#[tokio::test]
async fn thresholds_get_with_nonexistent_model() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "thresholdmodelnf@example.com")
        .await;
    let org = server.create_org(&user, "Threshold ModelNF Org").await;
    let project = server
        .create_project(&user, &org, "Threshold ModelNF Project")
        .await;
    let project_slug: &str = project.slug.as_ref();

    // Create threshold with a model
    let threshold = create_threshold_with_model(&server, &user, project_slug).await;

    // GET with a nonexistent model UUID should return 404
    let bogus_uuid = ModelUuid::new();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/thresholds/{}?model={}",
            project_slug, threshold.uuid, bogus_uuid
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// GET /v0/projects/{project}/thresholds/{threshold}?model=<other_threshold_model> - returns 404
#[tokio::test]
async fn thresholds_get_with_wrong_threshold_model() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "thresholdmodelwrong@example.com")
        .await;
    let org = server.create_org(&user, "Threshold ModelWrong Org").await;
    let project = server
        .create_project(&user, &org, "Threshold ModelWrong Project")
        .await;
    let project_slug: &str = project.slug.as_ref();

    // Create first threshold (with its own model)
    let threshold_a = create_threshold_with_model(&server, &user, project_slug).await;
    let model_a_uuid = threshold_a.model.as_ref().expect("model should exist").uuid;

    // Create a second branch/testbed/measure for a second threshold
    let body = serde_json::json!({
        "name": "other-branch",
        "slug": "other-branch"
    });
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/branches", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Create second threshold (reuse testbed and measure, different branch)
    let body = serde_json::json!({
        "branch": "other-branch",
        "testbed": "threshold-testbed",
        "measure": "throughput",
        "test": "z_score",
        "max_sample_size": 16,
        "upper_boundary": 0.95
    });
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/thresholds", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let threshold_b: JsonThreshold = resp.json().await.expect("Failed to parse threshold B");

    // GET threshold B with threshold A's model UUID should return 404
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/thresholds/{}?model={}",
            project_slug, threshold_b.uuid, model_a_uuid
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

async fn create_project_with_branch_testbed_measure(
    server: &TestServer,
    user: &TestUser,
    org_name: &str,
    project_name: &str,
) -> String {
    let org = server.create_org(user, org_name).await;
    let project = server.create_project(user, &org, project_name).await;
    let project_slug: String = AsRef::<str>::as_ref(&project.slug).to_owned();

    for (path, body) in [
        (
            "branches",
            serde_json::json!({"name": "ssize-branch", "slug": "ssize-branch"}),
        ),
        (
            "testbeds",
            serde_json::json!({"name": "ssize-testbed", "slug": "ssize-testbed"}),
        ),
        (
            "measures",
            serde_json::json!({
                "name": "latency",
                "slug": "latency",
                "units": "ns",
            }),
        ),
    ] {
        let resp = server
            .client
            .post(server.api_url(&format!("/v0/projects/{project_slug}/{path}")))
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(&user.token),
            )
            .json(&body)
            .send()
            .await
            .expect("Request failed");
        assert_eq!(resp.status(), StatusCode::CREATED);
    }

    project_slug
}

#[tokio::test]
async fn create_threshold_percentage_max_sample_size_one_succeeds() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "ssize-percentage@example.com")
        .await;
    let project_slug = create_project_with_branch_testbed_measure(
        &server,
        &user,
        "Sample Size Percentage Org",
        "Sample Size Percentage Project",
    )
    .await;

    let body = serde_json::json!({
        "branch": "ssize-branch",
        "testbed": "ssize-testbed",
        "measure": "latency",
        "test": "percentage",
        "max_sample_size": 1,
        "upper_boundary": 0.05,
    });
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{project_slug}/thresholds")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let _threshold: JsonThreshold = resp.json().await.expect("Failed to parse threshold");
}

#[tokio::test]
async fn create_threshold_t_test_max_sample_size_one_rejected() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "ssize-ttest@example.com").await;
    let project_slug = create_project_with_branch_testbed_measure(
        &server,
        &user,
        "Sample Size TTest Org",
        "Sample Size TTest Project",
    )
    .await;

    let body = serde_json::json!({
        "branch": "ssize-branch",
        "testbed": "ssize-testbed",
        "measure": "latency",
        "test": "t_test",
        "max_sample_size": 1,
        "upper_boundary": 0.99,
    });
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{project_slug}/thresholds")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert!(
        resp.status().is_client_error(),
        "Expected 4xx, got: {}",
        resp.status()
    );
}

#[tokio::test]
async fn create_threshold_iqr_max_sample_size_one_rejected() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "ssize-iqr@example.com").await;
    let project_slug = create_project_with_branch_testbed_measure(
        &server,
        &user,
        "Sample Size IQR Org",
        "Sample Size IQR Project",
    )
    .await;

    let body = serde_json::json!({
        "branch": "ssize-branch",
        "testbed": "ssize-testbed",
        "measure": "latency",
        "test": "iqr",
        "max_sample_size": 1,
        "upper_boundary": 1.5,
    });
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{project_slug}/thresholds")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert!(
        resp.status().is_client_error(),
        "Expected 4xx, got: {}",
        resp.status()
    );
}

#[cfg(feature = "plus")]
async fn post_json(
    server: &TestServer,
    user: &TestUser,
    path: &str,
    body: &serde_json::Value,
) -> (StatusCode, String) {
    let resp = server
        .client
        .post(server.api_url(path))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(body)
        .send()
        .await
        .expect("Request failed");
    let status = resp.status();
    let body = resp.text().await.expect("Failed to read the response");
    (status, body)
}

/// A project with `usage` thresholds, seeded through the threshold endpoint so that no report is
/// spent: one bare threshold, which a report can address, and the rest one per metric name.
#[cfg(feature = "plus")]
async fn project_with_thresholds(
    server: &TestServer,
    label: &str,
    usage: usize,
) -> (TestUser, String) {
    let user = server
        .signup("Test User", &format!("ceiling-{label}@example.com"))
        .await;
    let project_slug = create_project_with_branch_testbed_measure(
        server,
        &user,
        &format!("Ceiling {label} Org"),
        &format!("Ceiling {label} Project"),
    )
    .await;
    for n in 0..usage {
        let threshold = serde_json::json!({
            "branch": "ssize-branch",
            "testbed": "ssize-testbed",
            "measure": "latency",
            "metric": (n > 0).then(|| format!("p{n}")),
            "test": "percentage",
            "upper_boundary": 0.05,
        });
        let path = format!("/v0/projects/{project_slug}/thresholds");
        let (status, body) = post_json(server, &user, &path, &threshold).await;
        assert_eq!(status, StatusCode::CREATED, "seed threshold {n}: {body}");
    }
    (user, project_slug)
}

#[cfg(feature = "plus")]
async fn post_report(
    server: &TestServer,
    user: &TestUser,
    project_slug: &str,
    thresholds: Option<serde_json::Value>,
) -> (StatusCode, String) {
    let report = serde_json::json!({
        "branch": "ssize-branch",
        "testbed": "ssize-testbed",
        "start_time": "2024-01-01T00:00:00Z",
        "end_time": "2024-01-01T00:01:00Z",
        "results": ["{\"bench\": {\"latency\": {\"value\": 1.0}}}"],
        "thresholds": thresholds,
    });
    let path = format!("/v0/projects/{project_slug}/reports");
    post_json(server, user, &path, &report).await
}

/// Report thresholds that create one threshold on each of `count` new measures.
#[cfg(feature = "plus")]
fn new_thresholds(count: usize) -> serde_json::Value {
    let models = (1..=count)
        .map(|n| {
            (
                format!("measure-{n}"),
                serde_json::json!({ "test": "percentage", "upper_boundary": 0.05 }),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    serde_json::json!({ "models": models })
}

#[cfg(feature = "plus")]
async fn threshold_count(
    server: &TestServer,
    user: &TestUser,
    project_slug: &str,
    query: &str,
) -> usize {
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project_slug}/thresholds{query}")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let thresholds: JsonThresholds = resp.json().await.expect("Failed to parse thresholds");
    thresholds.0.len()
}

/// Every resource shares one limit, so the refusal has to name the threshold ceiling.
#[cfg(feature = "plus")]
fn assert_threshold_ceiling(status: StatusCode, body: &str) {
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    assert!(
        body.contains("for Threshold creation"),
        "the threshold ceiling fired: {body}"
    );
}

/// The API refuses a threshold past the ceiling, so this one goes straight into the database.
#[cfg(feature = "plus")]
fn insert_threshold_past_the_ceiling(server: &TestServer, project_slug: &str) {
    let project_id =
        ProjectId::try_from_raw(get_project_id(server, project_slug)).expect("valid project ID");
    let mut conn = server.db_conn();
    let QueryThreshold {
        branch_id,
        testbed_id,
        parameters,
        measure_id,
        created,
        modified,
        ..
    } = schema::threshold::table
        .filter(schema::threshold::project_id.eq(project_id))
        .first(&mut conn)
        .expect("Failed to load a seed threshold");
    diesel::insert_into(schema::threshold::table)
        .values(InsertThreshold {
            uuid: ThresholdUuid::new(),
            project_id,
            branch_id,
            testbed_id,
            parameters,
            measure_id,
            metric: Some("direct".parse().expect("valid metric name")),
            model_id: None,
            created,
            modified,
        })
        .execute(&mut conn)
        .expect("Failed to insert a threshold");
}

#[cfg(feature = "plus")]
#[tokio::test]
async fn threshold_ceiling_admits_a_report_that_creates_none() {
    let server = TestServer::new_with_creation_limits(4, 4).await;
    let (user, project_slug) = project_with_thresholds(&server, "none", 4).await;
    insert_threshold_past_the_ceiling(&server, &project_slug);
    assert_eq!(threshold_count(&server, &user, &project_slug, "").await, 5);

    let (status, body) = post_report(&server, &user, &project_slug, None).await;
    assert_eq!(status, StatusCode::CREATED, "no thresholds: {body}");

    let changed_model = serde_json::json!({
        "models": { "latency": { "test": "t_test", "upper_boundary": 0.99 } }
    });
    let (status, body) = post_report(&server, &user, &project_slug, Some(changed_model)).await;
    assert_eq!(status, StatusCode::CREATED, "a changed model: {body}");
}

#[cfg(feature = "plus")]
#[tokio::test]
async fn threshold_ceiling_refuses_a_report_whose_creates_pass_it() {
    let server = TestServer::new_with_creation_limits(4, 4).await;
    let (user, project_slug) = project_with_thresholds(&server, "over", 2).await;

    let (status, body) = post_report(&server, &user, &project_slug, Some(new_thresholds(3))).await;
    assert_threshold_ceiling(status, &body);
    assert_eq!(
        threshold_count(&server, &user, &project_slug, "").await,
        2,
        "the refused report created no threshold"
    );
}

#[cfg(feature = "plus")]
#[tokio::test]
async fn threshold_ceiling_admits_a_report_whose_creates_reach_it() {
    let server = TestServer::new_with_creation_limits(4, 4).await;
    let (user, project_slug) = project_with_thresholds(&server, "exact", 2).await;

    let (status, body) = post_report(&server, &user, &project_slug, Some(new_thresholds(2))).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(threshold_count(&server, &user, &project_slug, "").await, 4);
}

#[cfg(feature = "plus")]
#[tokio::test]
async fn threshold_ceiling_counts_a_start_point_clone() {
    let server = TestServer::new_with_creation_limits(5, 5).await;
    let (user, project_slug) = project_with_thresholds(&server, "clone", 2).await;
    // A start point is a version, so the branch needs a report to be cloned from.
    let (status, body) = post_report(&server, &user, &project_slug, None).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let path = format!("/v0/projects/{project_slug}/branches");
    let clone = |name: &str| {
        serde_json::json!({
            "name": name,
            "start_point": { "branch": "ssize-branch", "clone_thresholds": true }
        })
    };

    let (status, body) = post_json(&server, &user, &path, &clone("fits")).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(
        threshold_count(&server, &user, &project_slug, "?branch=fits").await,
        2
    );

    let (status, body) = post_json(&server, &user, &path, &clone("refused")).await;
    assert_threshold_ceiling(status, &body);
}
