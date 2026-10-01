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

use bencher_api_tests::{TestServer, TestUser, helpers::get_project_id};
#[cfg(feature = "plus")]
use bencher_json::JsonReportIterationCounts;
use bencher_json::{
    JsonReport, JsonThreshold, JsonThresholds, MetricName, ModelUuid, ThresholdUuid,
    project::{
        report::{JsonReportWarning, ReportWarningAction, ReportWarningResource},
        threshold::MAX_ACTIVE_THRESHOLDS,
    },
};
use bencher_schema::{
    model::project::{
        ProjectId,
        branch::BranchId,
        threshold::{
            InsertThreshold, QueryThreshold, ThresholdId,
            model::{InsertModel, ModelId, QueryModel},
        },
    },
    schema,
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _, SelectableHelper as _};
use http::{Method, StatusCode};

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

async fn send_json(
    server: &TestServer,
    user: &TestUser,
    method: Method,
    path: &str,
    body: &serde_json::Value,
) -> (StatusCode, String) {
    let resp = server
        .client
        .request(method, server.api_url(path))
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
        let (status, body) = send_json(server, &user, Method::POST, &path, &threshold).await;
        assert_eq!(status, StatusCode::CREATED, "seed threshold {n}: {body}");
    }
    (user, project_slug)
}

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
    send_json(server, user, Method::POST, &path, &report).await
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

/// The report was created with its one result, whatever became of its thresholds.
#[cfg(feature = "plus")]
fn assert_report_results(status: StatusCode, body: &str) {
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let report: JsonReport = serde_json::from_str(body).expect("Failed to parse the report");
    assert_eq!(
        report.counts.results,
        vec![JsonReportIterationCounts {
            benchmarks: 1,
            measures: 1,
        }],
        "the report kept its results"
    );
}

fn project_id(server: &TestServer, project_slug: &str) -> ProjectId {
    ProjectId::try_from_raw(get_project_id(server, project_slug)).expect("valid project ID")
}

/// The API refuses a threshold past the ceiling, so this one goes straight into the database.
#[cfg(feature = "plus")]
fn insert_threshold_past_the_ceiling(server: &TestServer, project_slug: &str) {
    let project_id = project_id(server, project_slug);
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

/// The API stops at the cap on active thresholds, so these go straight into the database: `count`
/// more thresholds with a model beside the first seed threshold, each copying its model.
///
/// They go in by descending UUID, so their row order is not their UUID order.
fn insert_active_thresholds(server: &TestServer, project_slug: &str, count: usize) {
    let project_id = project_id(server, project_slug);
    let mut conn = server.db_conn();
    let seed: QueryThreshold = schema::threshold::table
        .filter(schema::threshold::project_id.eq(project_id))
        .order(schema::threshold::id.asc())
        .first(&mut conn)
        .expect("Failed to load a seed threshold");
    let model = schema::model::table
        .filter(schema::model::threshold_id.eq(seed.id))
        .select(QueryModel::as_select())
        .first(&mut conn)
        .expect("Failed to load the seed model");
    let mut uuids = std::iter::repeat_with(ThresholdUuid::new)
        .take(count)
        .collect::<Vec<_>>();
    uuids.sort_unstable_by(|left, right| right.cmp(left));
    for (n, uuid) in uuids.into_iter().enumerate() {
        diesel::insert_into(schema::threshold::table)
            .values(InsertThreshold {
                uuid,
                project_id,
                branch_id: seed.branch_id,
                testbed_id: seed.testbed_id,
                parameters: None,
                measure_id: seed.measure_id,
                metric: Some(format!("direct-{n}").parse().expect("valid metric name")),
                model_id: None,
                created: seed.created,
                modified: seed.modified,
            })
            .execute(&mut conn)
            .expect("Failed to insert a threshold");
        let threshold_id: ThresholdId = schema::threshold::table
            .filter(schema::threshold::uuid.eq(uuid))
            .select(schema::threshold::id)
            .first(&mut conn)
            .expect("Failed to load the threshold id");
        let insert_model = InsertModel::with_threshold_id(model, threshold_id);
        diesel::insert_into(schema::model::table)
            .values(&insert_model)
            .execute(&mut conn)
            .expect("Failed to insert a model");
        let model_id: ModelId = schema::model::table
            .filter(schema::model::uuid.eq(insert_model.uuid))
            .select(schema::model::id)
            .first(&mut conn)
            .expect("Failed to load the model id");
        diesel::update(schema::threshold::table.filter(schema::threshold::id.eq(threshold_id)))
            .set(schema::threshold::model_id.eq(model_id))
            .execute(&mut conn)
            .expect("Failed to give the threshold its model");
    }
}

/// Every threshold on a branch: its UUID, the metric name it checks, and whether it carries a
/// model.
fn branch_thresholds(
    server: &TestServer,
    project_slug: &str,
    branch: &str,
) -> Vec<(ThresholdUuid, String, bool)> {
    let project_id = project_id(server, project_slug);
    let mut conn = server.db_conn();
    let branch_id: BranchId = schema::branch::table
        .filter(schema::branch::project_id.eq(project_id))
        .filter(schema::branch::name.eq(branch))
        .select(schema::branch::id)
        .first(&mut conn)
        .expect("Failed to load the branch");
    schema::threshold::table
        .filter(schema::threshold::branch_id.eq(branch_id))
        .select((
            schema::threshold::uuid,
            schema::threshold::metric,
            schema::threshold::model_id,
        ))
        .load::<(ThresholdUuid, Option<MetricName>, Option<ModelId>)>(&mut conn)
        .expect("Failed to load the thresholds")
        .into_iter()
        .map(|(uuid, metric, model_id)| {
            (
                uuid,
                metric.map_or_else(|| "value".to_owned(), |metric| metric.to_string()),
                model_id.is_some(),
            )
        })
        .collect()
}

/// The UUIDs of every threshold in a project.
async fn threshold_uuids(
    server: &TestServer,
    user: &TestUser,
    project_slug: &str,
) -> Vec<ThresholdUuid> {
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{project_slug}/thresholds?per_page=255"
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let thresholds: JsonThresholds = resp.json().await.expect("Failed to parse thresholds");
    thresholds
        .0
        .into_iter()
        .map(|threshold| threshold.uuid)
        .collect()
}

fn new_threshold(metric: &str) -> serde_json::Value {
    serde_json::json!({
        "branch": "ssize-branch",
        "testbed": "ssize-testbed",
        "measure": "latency",
        "metric": metric,
        "test": "percentage",
        "upper_boundary": 0.05,
    })
}

// A branch, testbed, and measure carry at most `MAX_ACTIVE_THRESHOLDS` thresholds with a model, and
// one whose model was removed no longer counts.
#[tokio::test]
async fn active_cap_refuses_a_create_past_it() {
    let server = TestServer::new().await;
    let (user, project_slug) =
        project_with_thresholds(&server, "capcreate", MAX_ACTIVE_THRESHOLDS).await;
    let path = format!("/v0/projects/{project_slug}/thresholds");

    let (status, body) =
        send_json(&server, &user, Method::POST, &path, &new_threshold("past")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    let stripped = threshold_uuids(&server, &user, &project_slug)
        .await
        .into_iter()
        .next()
        .expect("a seed threshold");
    let remove = serde_json::json!({ "test": null });
    let (status, body) = send_json(
        &server,
        &user,
        Method::PUT,
        &format!("{path}/{stripped}"),
        &remove,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, body) =
        send_json(&server, &user, Method::POST, &path, &new_threshold("past")).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
}

// Giving a model to a threshold that has none counts toward the cap, and changing the model of
// one that has a model does not.
#[tokio::test]
async fn active_cap_refuses_to_give_a_model_past_it() {
    let server = TestServer::new().await;
    let (user, project_slug) =
        project_with_thresholds(&server, "capupdate", MAX_ACTIVE_THRESHOLDS).await;
    let path = format!("/v0/projects/{project_slug}/thresholds");
    let mut uuids = threshold_uuids(&server, &user, &project_slug)
        .await
        .into_iter();
    let stripped = uuids.next().expect("a seed threshold");
    let active = uuids.next().expect("another seed threshold");

    let remove = serde_json::json!({ "test": null });
    let (status, body) = send_json(
        &server,
        &user,
        Method::PUT,
        &format!("{path}/{stripped}"),
        &remove,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) =
        send_json(&server, &user, Method::POST, &path, &new_threshold("past")).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let model = serde_json::json!({ "test": "t_test", "upper_boundary": 0.99 });
    let (status, body) = send_json(
        &server,
        &user,
        Method::PUT,
        &format!("{path}/{stripped}"),
        &model,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let (status, body) = send_json(
        &server,
        &user,
        Method::PUT,
        &format!("{path}/{active}"),
        &model,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

// A start point clone copies what fits under the cap, in the UUID order of the thresholds it
// copies, and skips the rest.
#[tokio::test]
async fn a_start_point_clone_past_the_active_cap_copies_what_fits_in_uuid_order() {
    let server = TestServer::new().await;
    let (user, project_slug) = project_with_thresholds(&server, "capclone", 1).await;
    // More than one past the cap, so that any other order copies a different set.
    insert_active_thresholds(&server, &project_slug, MAX_ACTIVE_THRESHOLDS + 3);
    // A start point is a version, so the branch needs a report to be cloned from.
    let (status, body) = post_report(&server, &user, &project_slug, None).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let mut source = branch_thresholds(&server, &project_slug, "ssize-branch");
    source.sort();
    let mut fits = source
        .into_iter()
        .take(MAX_ACTIVE_THRESHOLDS)
        .map(|(_, metric, has_model)| (metric, has_model))
        .collect::<Vec<_>>();
    fits.sort();

    let clone = serde_json::json!({
        "name": "capped",
        "start_point": { "branch": "ssize-branch", "clone_thresholds": true }
    });
    let path = format!("/v0/projects/{project_slug}/branches");
    let (status, body) = send_json(&server, &user, Method::POST, &path, &clone).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let mut copied = branch_thresholds(&server, &project_slug, "capped")
        .into_iter()
        .map(|(_, metric, has_model)| (metric, has_model))
        .collect::<Vec<_>>();
    copied.sort();
    assert_eq!(copied, fits);
}

// A report's start point clone that passes the cap warns of what it skipped on that report, when the
// report creates the branch and when it resets the branch's head.
#[tokio::test]
async fn a_report_whose_start_point_clone_passes_the_active_cap_warns_of_what_it_skipped() {
    let server = TestServer::new().await;
    let (user, project_slug) = project_with_thresholds(&server, "capclonewarn", 1).await;
    insert_active_thresholds(&server, &project_slug, MAX_ACTIVE_THRESHOLDS + 3);
    // A start point is a version, so the branch needs a report to be cloned from.
    let (status, body) = post_report(&server, &user, &project_slug, None).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    // The seed and the inserted thresholds are four past the cap.
    let skipped = Some(vec![JsonReportWarning {
        resource: ReportWarningResource::Threshold,
        action: ReportWarningAction::Skip,
        count: 4,
    }]);
    let path = format!("/v0/projects/{project_slug}/reports");
    for reset in [false, true] {
        let report = serde_json::json!({
            "branch": "feature",
            "start_point": { "branch": "ssize-branch", "clone_thresholds": true, "reset": reset },
            "testbed": "ssize-testbed",
            "start_time": "2024-01-01T00:00:00Z",
            "end_time": "2024-01-01T00:01:00Z",
            "results": ["{\"bench\": {\"latency\": {\"value\": 1.0}}}"],
        });
        let (status, body) = send_json(&server, &user, Method::POST, &path, &report).await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        let report: JsonReport = serde_json::from_str(&body).expect("Failed to parse the report");
        assert_eq!(report.warnings, skipped, "reset: {reset}");
    }
}

// A start point reset clones onto a branch that already has thresholds. What the clone strips
// makes room under the cap, and a threshold it gives a model back counts toward it in UUID order.
#[tokio::test]
async fn a_start_point_reset_clone_frees_room_under_the_active_cap() {
    let server = TestServer::new().await;
    let (user, project_slug) = project_with_thresholds(&server, "capreset", 2).await;
    // A start point is a version, so the branch needs a report to be cloned from.
    let (status, body) = post_report(&server, &user, &project_slug, None).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let branches = format!("/v0/projects/{project_slug}/branches");
    let clone = serde_json::json!({
        "name": "feature",
        "start_point": { "branch": "ssize-branch", "clone_thresholds": true }
    });
    let (status, body) = send_json(&server, &user, Method::POST, &branches, &clone).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    insert_active_thresholds(&server, &project_slug, MAX_ACTIVE_THRESHOLDS + 3);

    // On the feature branch, `value` keeps a model its twin loses, `p1` loses a model its twin
    // keeps, and `orphan` has no twin.
    let thresholds = format!("/v0/projects/{project_slug}/thresholds");
    let uuid = |branch: &str, metric: &str| {
        branch_thresholds(&server, &project_slug, branch)
            .into_iter()
            .find(|(_, name, _)| name == metric)
            .map(|(uuid, ..)| uuid)
            .expect("Failed to find the threshold")
    };
    let remove = serde_json::json!({ "test": null });
    for stripped in [uuid("ssize-branch", "value"), uuid("feature", "p1")] {
        let path = format!("{thresholds}/{stripped}");
        let (status, body) = send_json(&server, &user, Method::PUT, &path, &remove).await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }
    let mut orphan = new_threshold("orphan");
    orphan["branch"] = "feature".into();
    let (status, body) = send_json(&server, &user, Method::POST, &thresholds, &orphan).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let mut source = branch_thresholds(&server, &project_slug, "ssize-branch")
        .into_iter()
        .filter(|(_, _, has_model)| *has_model)
        .collect::<Vec<_>>();
    source.sort();
    let mut fits = source
        .into_iter()
        .take(MAX_ACTIVE_THRESHOLDS)
        .map(|(_, metric, _)| metric)
        .collect::<Vec<_>>();
    fits.sort();

    let reset = serde_json::json!({
        "start_point": { "branch": "ssize-branch", "clone_thresholds": true, "reset": true }
    });
    let path = format!("{branches}/feature");
    let (status, body) = send_json(&server, &user, Method::PATCH, &path, &reset).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let mut active = branch_thresholds(&server, &project_slug, "feature")
        .into_iter()
        .filter(|(_, _, has_model)| *has_model)
        .map(|(_, metric, _)| metric)
        .collect::<Vec<_>>();
    active.sort();
    assert_eq!(active, fits);
}

// The API stays strict past the ceiling, where a report skips.
#[cfg(feature = "plus")]
#[tokio::test]
async fn threshold_ceiling_refuses_an_api_create_past_it() {
    let server = TestServer::new_with_creation_limits(4, 4).await;
    let (user, project_slug) = project_with_thresholds(&server, "api", 4).await;

    let path = format!("/v0/projects/{project_slug}/thresholds");
    let (status, body) =
        send_json(&server, &user, Method::POST, &path, &new_threshold("past")).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    assert!(
        body.contains("for Threshold creation"),
        "the threshold ceiling fired: {body}"
    );
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
async fn threshold_ceiling_skips_every_create_of_a_report_that_passes_it() {
    let server = TestServer::new_with_creation_limits(4, 4).await;
    let (user, project_slug) = project_with_thresholds(&server, "over", 2).await;

    let (status, body) = post_report(&server, &user, &project_slug, Some(new_thresholds(3))).await;
    assert_report_results(status, &body);
    assert_eq!(
        threshold_count(&server, &user, &project_slug, "").await,
        2,
        "the report created no threshold"
    );
}

// Over the ceiling a report still applies what creates nothing.
#[cfg(feature = "plus")]
#[tokio::test]
async fn threshold_ceiling_skips_the_creates_of_a_report_but_applies_its_update() {
    let server = TestServer::new_with_creation_limits(4, 4).await;
    let (user, project_slug) = project_with_thresholds(&server, "update", 2).await;
    let project_id = project_id(&server, &project_slug);
    let mut conn = server.db_conn();
    let bare: ThresholdId = schema::threshold::table
        .filter(schema::threshold::project_id.eq(project_id))
        .filter(schema::threshold::metric.is_null())
        .select(schema::threshold::id)
        .first(&mut conn)
        .expect("Failed to load the bare threshold");
    drop(conn);

    let mut thresholds = new_thresholds(3);
    thresholds["models"]["latency"] =
        serde_json::json!({ "test": "t_test", "upper_boundary": 0.99 });
    let (status, body) = post_report(&server, &user, &project_slug, Some(thresholds)).await;
    assert_report_results(status, &body);
    assert_eq!(
        threshold_count(&server, &user, &project_slug, "").await,
        2,
        "the report created no threshold"
    );

    let mut conn = server.db_conn();
    let models = schema::model::table
        .filter(schema::model::threshold_id.eq(bare))
        .count()
        .get_result::<i64>(&mut conn)
        .expect("Failed to count the models");
    assert_eq!(models, 2, "the report updated the model it named");
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
async fn threshold_ceiling_warns_a_report_whose_start_point_clone_passes_it() {
    let server = TestServer::new_with_creation_limits(4, 4).await;
    let (user, project_slug) = project_with_thresholds(&server, "clonewarn", 3).await;
    // A start point is a version, so the branch needs a report to be cloned from.
    let (status, body) = post_report(&server, &user, &project_slug, None).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let report = serde_json::json!({
        "branch": "feature",
        "start_point": { "branch": "ssize-branch", "clone_thresholds": true },
        "testbed": "ssize-testbed",
        "start_time": "2024-01-01T00:00:00Z",
        "end_time": "2024-01-01T00:01:00Z",
        "results": ["{\"bench\": {\"latency\": {\"value\": 1.0}}}"],
    });
    let path = format!("/v0/projects/{project_slug}/reports");
    let (status, body) = send_json(&server, &user, Method::POST, &path, &report).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let report: JsonReport = serde_json::from_str(&body).expect("Failed to parse the report");
    assert_eq!(
        report.warnings,
        Some(vec![JsonReportWarning {
            resource: ReportWarningResource::Threshold,
            action: ReportWarningAction::Skip,
            count: 3,
        }])
    );
}

#[cfg(feature = "plus")]
#[tokio::test]
async fn threshold_ceiling_skips_a_start_point_clone_that_passes_it() {
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

    let (status, body) = send_json(&server, &user, Method::POST, &path, &clone("fits")).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(
        threshold_count(&server, &user, &project_slug, "?branch=fits").await,
        2
    );

    let (status, body) = send_json(&server, &user, Method::POST, &path, &clone("skipped")).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(
        threshold_count(&server, &user, &project_slug, "?branch=skipped").await,
        0,
        "the clone copied no threshold"
    );
}
