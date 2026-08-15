#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args,
    reason = "integration test file"
)]
//! Integration tests for project benchmark endpoints.

use bencher_api_tests::{TestServer, helpers::create_empty_parameter};
use bencher_json::{JsonBenchmark, JsonBenchmarks, JsonParameters};
use bencher_schema::schema;
use diesel::{
    ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _, connection::SimpleConnection as _,
};
use http::StatusCode;

// GET /v0/projects/{project}/benchmarks - list benchmarks (empty)
#[tokio::test]
async fn benchmarks_list_empty() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "benchmarklist@example.com")
        .await;
    let org = server.create_org(&user, "Benchmark Org").await;
    let project = server
        .create_project(&user, &org, "Benchmark Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}/benchmarks", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let benchmarks: JsonBenchmarks = resp.json().await.expect("Failed to parse response");
    // New project should have no benchmarks
    assert!(benchmarks.0.is_empty());
}

// GET /v0/projects/{project}/benchmarks - with search
#[tokio::test]
async fn benchmarks_list_with_search() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "benchmarksearch@example.com")
        .await;
    let org = server.create_org(&user, "Benchmark Search Org").await;
    let project = server
        .create_project(&user, &org, "Benchmark Search Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/benchmarks?search=foo",
            project_slug
        )))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
}

// GET /v0/projects/{project}/benchmarks/{benchmark} - not found
#[tokio::test]
async fn benchmarks_get_not_found() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "benchmarknotfound@example.com")
        .await;
    let org = server.create_org(&user, "Benchmark NotFound Org").await;
    let project = server
        .create_project(&user, &org, "Benchmark NotFound Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/benchmarks/nonexistent-benchmark",
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

// DELETE /v0/projects/{project}/benchmarks/{benchmark} - not found
#[tokio::test]
async fn benchmarks_delete_not_found() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "benchmarkdelnotfound@example.com")
        .await;
    let org = server.create_org(&user, "Benchmark Del NotFound Org").await;
    let project = server
        .create_project(&user, &org, "Benchmark Del NotFound Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .delete(server.api_url(&format!(
            "/v0/projects/{}/benchmarks/nonexistent-benchmark",
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

// Every benchmark is born with exactly one empty parameter set, so no benchmark
// row may exist without one.
#[expect(clippy::expect_used, reason = "test assertion")]
fn assert_benchmark_birth_invariant(server: &TestServer) {
    let mut conn = server.db_conn();
    let benchmark_ids: Vec<i32> = schema::benchmark::table
        .select(schema::benchmark::id)
        .load(&mut conn)
        .expect("Failed to load benchmarks");
    assert!(
        !benchmark_ids.is_empty(),
        "expected at least one benchmark to check"
    );

    for benchmark_id in benchmark_ids {
        let parameters: Vec<JsonParameters> = schema::parameter::table
            .filter(schema::parameter::benchmark_id.eq(benchmark_id))
            .select(schema::parameter::parameters)
            .load(&mut conn)
            .expect("Failed to load parameters");
        assert_eq!(
            parameters,
            vec![JsonParameters::default()],
            "benchmark {benchmark_id} must have exactly one empty parameter set"
        );
    }
}

// POST /v0/projects/{project}/benchmarks - create with the empty parameter set
#[tokio::test]
async fn benchmarks_create_empty_parameter_set() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "benchmarkparameter@example.com")
        .await;
    let org = server.create_org(&user, "Benchmark Parameter Org").await;
    let project = server
        .create_project(&user, &org, "Benchmark Parameter Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/benchmarks", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&serde_json::json!({ "name": "bench one" }))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
    let benchmark: JsonBenchmark = resp.json().await.expect("Failed to parse response");

    let mut conn = server.db_conn();
    let benchmark_id: i32 = schema::benchmark::table
        .filter(schema::benchmark::uuid.eq(benchmark.uuid))
        .select(schema::benchmark::id)
        .first(&mut conn)
        .expect("Failed to get benchmark ID");
    let parameters: Vec<JsonParameters> = schema::parameter::table
        .filter(schema::parameter::benchmark_id.eq(benchmark_id))
        .select(schema::parameter::parameters)
        .load(&mut conn)
        .expect("Failed to load parameters");
    assert_eq!(parameters, vec![JsonParameters::default()]);

    assert_benchmark_birth_invariant(&server);
}

// POST /v0/projects/{project}/benchmarks - the benchmark insert rolls back with
// the empty parameter set insert
#[tokio::test]
async fn benchmarks_create_rolls_back_with_parameter_set() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "benchmarkrollback@example.com")
        .await;
    let org = server.create_org(&user, "Benchmark Rollback Org").await;
    let project = server
        .create_project(&user, &org, "Benchmark Rollback Project")
        .await;

    // Poison the empty parameter set that the next benchmark will be born with.
    // SQLite hands an `INTEGER PRIMARY KEY` the next rowid after the largest in
    // use, so the row below collides on `UNIQUE(benchmark_id, parameters)` with
    // the set created inside the benchmark's own transaction. Foreign keys are
    // off on this connection, so it may point at a benchmark that does not exist yet.
    let mut conn = server.db_conn();
    let largest_benchmark_id: Option<i32> = schema::benchmark::table
        .select(diesel::dsl::max(schema::benchmark::id))
        .first(&mut conn)
        .expect("Failed to get the largest benchmark ID");
    let next_benchmark_id = largest_benchmark_id.unwrap_or_default() + 1;
    conn.batch_execute("PRAGMA foreign_keys = OFF")
        .expect("Failed to disable foreign keys");
    create_empty_parameter(&mut conn, next_benchmark_id);
    conn.batch_execute("PRAGMA foreign_keys = ON")
        .expect("Failed to enable foreign keys");

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/benchmarks", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&serde_json::json!({ "name": "bench one" }))
        .send()
        .await
        .expect("Request failed");
    assert!(
        !resp.status().is_success(),
        "creating a benchmark whose parameter set collides must fail"
    );

    let benchmarks: i64 = schema::benchmark::table
        .filter(schema::benchmark::name.eq("bench one"))
        .count()
        .get_result(&mut conn)
        .expect("Failed to count benchmarks");
    assert_eq!(
        benchmarks, 0,
        "the benchmark insert must roll back with its parameter set insert"
    );
}
