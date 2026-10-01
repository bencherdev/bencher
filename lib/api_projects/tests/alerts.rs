#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::tests_outside_test_module,
    clippy::too_many_lines,
    clippy::uninlined_format_args,
    reason = "integration test file"
)]
//! Integration tests for project alert endpoints.

use bencher_api_tests::{
    TestServer, TestUser,
    helpers::{base_timestamp, create_empty_variant, get_project_id},
};
use bencher_json::{
    AlertUuid, BenchmarkUuid, BoundaryUuid, BranchUuid, DateTime, HeadUuid, JsonAlerts,
    MeasureUuid, MetricName, MetricUuid, ModelUuid, ReportBenchmarkUuid, ReportUuid, TestbedUuid,
    ThresholdUuid, VersionUuid,
    project::{alert::AlertStatus, boundary::BoundaryLimit},
};
use bencher_schema::{MIGRATIONS, context::DbConnection, macros::sql::last_insert_rowid, schema};
use diesel::{
    ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _, connection::SimpleConnection as _,
};
use diesel_migrations::MigrationHarness as _;
use http::StatusCode;

// GET /v0/projects/{project}/alerts - list alerts (empty)
#[tokio::test]
async fn alerts_list_empty() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "alertlist@example.com").await;
    let org = server.create_org(&user, "Alert Org").await;
    let project = server.create_project(&user, &org, "Alert Project").await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}/alerts", project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let alerts: JsonAlerts = resp.json().await.expect("Failed to parse response");
    // New project should have no alerts
    assert!(alerts.0.is_empty());
}

// GET /v0/projects/{project}/alerts - with pagination
#[tokio::test]
async fn alerts_list_with_pagination() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "alertpage@example.com").await;
    let org = server.create_org(&user, "Alert Page Org").await;
    let project = server
        .create_project(&user, &org, "Alert Page Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/alerts?per_page=10&page=1",
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

// GET /v0/projects/{project}/alerts/{alert} - not found
#[tokio::test]
async fn alerts_get_not_found() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "alertnotfound@example.com")
        .await;
    let org = server.create_org(&user, "Alert NotFound Org").await;
    let project = server
        .create_project(&user, &org, "Alert NotFound Project")
        .await;

    let project_slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!(
            "/v0/projects/{}/alerts/00000000-0000-0000-0000-000000000000",
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

#[tokio::test]
async fn alerts_list_and_count_by_project() {
    let server = TestServer::new().await;
    let a = seed_project(&server, "lista").await;
    let b = seed_project(&server, "listb").await;
    let a_alerts = insert_alerts(&server, &a);
    let b_alerts = insert_alerts(&server, &b);

    for (fixture, alerts) in [(&a, &a_alerts), (&b, &b_alerts)] {
        for (query, expected, total) in [
            ("", &[0, 2, 1, 3, 4, 5][..], 6),
            ("?direction=asc", &[1, 2, 0, 4, 3, 5], 6),
            ("?sort=created", &[1, 0, 2, 3, 4, 5], 6),
            ("?sort=created&direction=desc", &[2, 0, 1, 4, 3, 5], 6),
            ("?archived=true", &[6, 8, 7], 3),
            ("?status=dismissed", &[3, 4], 2),
            ("?archived=true&status=active", &[6, 8], 2),
            ("?per_page=2&page=2", &[1, 3], 6),
            ("?per_page=0&status=active", &[], 3),
        ] {
            let expected = expected
                .iter()
                .map(|&seed| alerts[seed])
                .collect::<Vec<_>>();
            assert_eq!(
                get_alerts(&server, &fixture.user, &fixture.slug, query).await,
                (expected, total),
                "{}{query}",
                fixture.slug
            );
        }
    }
}

#[tokio::test]
async fn migration_backfills_and_round_trips_every_alert() {
    let server = TestServer::new().await;
    let a = seed_project(&server, "migrationa").await;
    let b = seed_project(&server, "migrationb").await;

    let mut conn = server.db_conn();
    revert_migration(&mut conn);
    let mut expected = Vec::new();
    for fixture in [&b, &a] {
        for (seed, boundary) in SEEDS.iter().zip(&fixture.boundaries) {
            let uuid = AlertUuid::new();
            diesel::insert_into(schema::alert::table)
                .values((
                    schema::alert::uuid.eq(&uuid),
                    schema::alert::boundary_id.eq(boundary.boundary_id),
                    schema::alert::boundary_limit.eq(BoundaryLimit::Upper),
                    schema::alert::status.eq(seed.status),
                    schema::alert::modified.eq(seconds(seed.modified)),
                ))
                .execute(&mut conn)
                .expect("Failed to insert an alert without its project");
            expected.push((uuid, fixture.project_id, boundary.threshold_id));
        }
    }
    apply_migration(&mut conn);

    let alerts: Vec<(AlertUuid, i32, i32)> = schema::alert::table
        .order(schema::alert::id.asc())
        .select((
            schema::alert::uuid,
            schema::alert::project_id,
            schema::alert::threshold_id,
        ))
        .load(&mut conn)
        .expect("Failed to load the alerts");
    assert_eq!(alerts, expected);

    let before = capture(&server, &[&a, &b]).await;
    revert_migration(&mut conn);
    apply_migration(&mut conn);
    assert_eq!(capture(&server, &[&a, &b]).await, before);
}

#[tokio::test]
async fn alerts_belong_to_their_own_project() {
    let server = TestServer::new().await;
    // An unrelated project first, so no project's ID matches its threshold's or its branch's.
    let unrelated = server.signup("Test User", "unrelated@example.com").await;
    let unrelated_org = server.create_org(&unrelated, "Org unrelated").await;
    server
        .create_project(&unrelated, &unrelated_org, "Project unrelated")
        .await;
    let (a, a_slug, a_alert) = seed_alerting_project(&server, "owna").await;
    let (b, b_slug, b_alert) = seed_alerting_project(&server, "ownb").await;

    for (user, slug, alert) in [(&a, &a_slug, a_alert), (&b, &b_slug, b_alert)] {
        for (query, expected) in [
            ("?per_page=8&page=1", vec![alert]),
            ("?per_page=0&status=active", Vec::new()),
        ] {
            assert_eq!(
                get_alerts(&server, user, slug, query).await,
                (expected, 1),
                "{slug}{query}"
            );
        }
    }

    for (user, slug, own, other) in [
        (&a, &a_slug, a_alert, b_alert),
        (&b, &b_slug, b_alert, a_alert),
    ] {
        for (alert, status) in [(own, StatusCode::OK), (other, StatusCode::NOT_FOUND)] {
            let resp = server
                .client
                .get(server.api_url(&format!("/v0/projects/{slug}/alerts/{alert}")))
                .header(
                    bencher_json::AUTHORIZATION,
                    bencher_json::bearer_header(&user.token),
                )
                .send()
                .await
                .expect("Request failed");
            assert_eq!(resp.status(), status, "{slug} {alert}");
        }
    }
}

#[derive(Clone, Copy)]
enum Dimensions {
    Live,
    ArchivedBranch,
    ArchivedTestbed,
    ArchivedMeasure,
}

struct Seed {
    dimensions: Dimensions,
    status: AlertStatus,
    modified: i64,
    created: i64,
}

/// The alerts every seeded project holds, named by index in the tests. Within each
/// status the order by modification and the order by creation differ, in either
/// direction.
const SEEDS: [Seed; 9] = [
    seed(Dimensions::Live, AlertStatus::Active, 3, 2),
    seed(Dimensions::Live, AlertStatus::Active, 1, 1),
    seed(Dimensions::Live, AlertStatus::Active, 2, 3),
    seed(Dimensions::Live, AlertStatus::Dismissed, 5, 5),
    seed(Dimensions::Live, AlertStatus::Dismissed, 4, 6),
    seed(Dimensions::Live, AlertStatus::Silenced, 6, 0),
    seed(Dimensions::ArchivedBranch, AlertStatus::Active, 8, 7),
    seed(Dimensions::ArchivedTestbed, AlertStatus::Dismissed, 9, 8),
    seed(Dimensions::ArchivedMeasure, AlertStatus::Active, 7, 9),
];

const fn seed(dimensions: Dimensions, status: AlertStatus, modified: i64, created: i64) -> Seed {
    Seed {
        dimensions,
        status,
        modified,
        created,
    }
}

struct Fixture {
    user: TestUser,
    slug: String,
    project_id: i32,
    boundaries: Vec<SeededBoundary>,
}

struct SeededBoundary {
    boundary_id: i32,
    threshold_id: i32,
}

/// The live threshold's model was replaced after the first seed's boundary, so not
/// every boundary's model shares its threshold's ID.
async fn seed_project(server: &TestServer, label: &str) -> Fixture {
    let user = server
        .signup("Test User", &format!("{label}@example.com"))
        .await;
    let org = server.create_org(&user, &format!("Org {label}")).await;
    let project = server
        .create_project(&user, &org, &format!("Project {label}"))
        .await;
    let slug = project.slug.to_string();
    let project_id = get_project_id(server, &slug);
    let archived = Some(seconds(0));

    let mut conn = server.db_conn();
    diesel::insert_into(schema::version::table)
        .values((
            schema::version::uuid.eq(VersionUuid::new()),
            schema::version::project_id.eq(project_id),
            schema::version::number.eq(1),
        ))
        .execute(&mut conn)
        .expect("Failed to insert the version");
    let version_id = last_id(&mut conn);
    let live_branch = insert_branch(&mut conn, project_id, version_id, "live", None);
    let archived_branch = insert_branch(&mut conn, project_id, version_id, "gone", archived);
    let live_testbed = insert_testbed(&mut conn, project_id, "live", None);
    let archived_testbed = insert_testbed(&mut conn, project_id, "gone", archived);
    let live_measure = insert_measure(&mut conn, project_id, "live", None);
    let archived_measure = insert_measure(&mut conn, project_id, "gone", archived);
    diesel::insert_into(schema::benchmark::table)
        .values((
            schema::benchmark::uuid.eq(BenchmarkUuid::new()),
            schema::benchmark::project_id.eq(project_id),
            schema::benchmark::name.eq("bench"),
            schema::benchmark::slug.eq("bench"),
            schema::benchmark::created.eq(seconds(0)),
            schema::benchmark::modified.eq(seconds(0)),
        ))
        .execute(&mut conn)
        .expect("Failed to insert the benchmark");
    let benchmark_id = last_id(&mut conn);
    let variant_id = create_empty_variant(&mut conn, benchmark_id);

    let live = insert_threshold(
        &mut conn,
        project_id,
        live_branch,
        live_testbed,
        live_measure,
    );
    let replaced_model = live.model;
    diesel::update(schema::model::table.filter(schema::model::id.eq(replaced_model)))
        .set(schema::model::replaced.eq(seconds(0)))
        .execute(&mut conn)
        .expect("Failed to replace the live model");
    let live = SeededThreshold {
        model: insert_model(&mut conn, live.id),
        ..live
    };
    let on_archived_branch = insert_threshold(
        &mut conn,
        project_id,
        archived_branch,
        live_testbed,
        live_measure,
    );
    let on_archived_testbed = insert_threshold(
        &mut conn,
        project_id,
        live_branch,
        archived_testbed,
        live_measure,
    );
    let on_archived_measure = insert_threshold(
        &mut conn,
        project_id,
        live_branch,
        live_testbed,
        archived_measure,
    );

    let mut boundaries = Vec::with_capacity(SEEDS.len());
    for (index, seed) in SEEDS.iter().enumerate() {
        let threshold = match seed.dimensions {
            Dimensions::Live => live,
            Dimensions::ArchivedBranch => on_archived_branch,
            Dimensions::ArchivedTestbed => on_archived_testbed,
            Dimensions::ArchivedMeasure => on_archived_measure,
        };
        let model_id = if index == 0 {
            replaced_model
        } else {
            threshold.model
        };
        let created = seconds(seed.created);
        diesel::insert_into(schema::report::table)
            .values((
                schema::report::uuid.eq(ReportUuid::new()),
                schema::report::project_id.eq(project_id),
                schema::report::head_id.eq(threshold.head),
                schema::report::version_id.eq(version_id),
                schema::report::testbed_id.eq(threshold.testbed),
                schema::report::adapter.eq(0),
                schema::report::start_time.eq(&created),
                schema::report::end_time.eq(&created),
                schema::report::created.eq(&created),
            ))
            .execute(&mut conn)
            .expect("Failed to insert a report");
        let report_id = last_id(&mut conn);
        diesel::insert_into(schema::report_benchmark::table)
            .values((
                schema::report_benchmark::uuid.eq(ReportBenchmarkUuid::new()),
                schema::report_benchmark::report_id.eq(report_id),
                schema::report_benchmark::iteration.eq(0),
                schema::report_benchmark::benchmark_id.eq(benchmark_id),
                schema::report_benchmark::variant_id.eq(variant_id),
            ))
            .execute(&mut conn)
            .expect("Failed to insert a report benchmark");
        let report_benchmark_id = last_id(&mut conn);
        diesel::insert_into(schema::metric::table)
            .values((
                schema::metric::uuid.eq(MetricUuid::new()),
                schema::metric::report_benchmark_id.eq(report_benchmark_id),
                schema::metric::measure_id.eq(threshold.measure),
                schema::metric::name.eq(MetricName::value()),
                schema::metric::value.eq(1000.0),
            ))
            .execute(&mut conn)
            .expect("Failed to insert a metric");
        let metric_id = last_id(&mut conn);
        diesel::insert_into(schema::boundary::table)
            .values((
                schema::boundary::uuid.eq(BoundaryUuid::new()),
                schema::boundary::metric_id.eq(metric_id),
                schema::boundary::threshold_id.eq(threshold.id),
                schema::boundary::model_id.eq(model_id),
                schema::boundary::baseline.eq(Some(1.0)),
                schema::boundary::upper_limit.eq(Some(100.0)),
            ))
            .execute(&mut conn)
            .expect("Failed to insert a boundary");
        boundaries.push(SeededBoundary {
            boundary_id: last_id(&mut conn),
            threshold_id: threshold.id,
        });
    }

    Fixture {
        user,
        slug,
        project_id,
        boundaries,
    }
}

fn insert_alerts(server: &TestServer, fixture: &Fixture) -> Vec<AlertUuid> {
    let mut conn = server.db_conn();
    SEEDS
        .iter()
        .zip(&fixture.boundaries)
        .map(|(seed, boundary)| {
            let uuid = AlertUuid::new();
            diesel::insert_into(schema::alert::table)
                .values((
                    schema::alert::uuid.eq(&uuid),
                    schema::alert::project_id.eq(fixture.project_id),
                    schema::alert::threshold_id.eq(boundary.threshold_id),
                    schema::alert::boundary_id.eq(boundary.boundary_id),
                    schema::alert::boundary_limit.eq(BoundaryLimit::Upper),
                    schema::alert::status.eq(seed.status),
                    schema::alert::modified.eq(seconds(seed.modified)),
                ))
                .execute(&mut conn)
                .expect("Failed to insert an alert");
            uuid
        })
        .collect()
}

#[derive(Clone, Copy)]
struct SeededThreshold {
    id: i32,
    model: i32,
    head: i32,
    testbed: i32,
    measure: i32,
}

fn insert_branch(
    conn: &mut DbConnection,
    project_id: i32,
    version_id: i32,
    name: &str,
    archived: Option<DateTime>,
) -> (i32, i32) {
    diesel::insert_into(schema::branch::table)
        .values((
            schema::branch::uuid.eq(BranchUuid::new()),
            schema::branch::project_id.eq(project_id),
            schema::branch::name.eq(name),
            schema::branch::slug.eq(name),
            schema::branch::created.eq(seconds(0)),
            schema::branch::modified.eq(seconds(0)),
            schema::branch::archived.eq(archived),
        ))
        .execute(&mut *conn)
        .expect("Failed to insert a branch");
    let branch_id = last_id(conn);
    diesel::insert_into(schema::head::table)
        .values((
            schema::head::uuid.eq(HeadUuid::new()),
            schema::head::branch_id.eq(branch_id),
            schema::head::created.eq(seconds(0)),
        ))
        .execute(&mut *conn)
        .expect("Failed to insert a head");
    let head_id = last_id(conn);
    diesel::update(schema::branch::table.filter(schema::branch::id.eq(branch_id)))
        .set(schema::branch::head_id.eq(head_id))
        .execute(&mut *conn)
        .expect("Failed to point the branch at its head");
    diesel::insert_into(schema::head_version::table)
        .values((
            schema::head_version::head_id.eq(head_id),
            schema::head_version::version_id.eq(version_id),
        ))
        .execute(&mut *conn)
        .expect("Failed to insert a head version");
    (branch_id, head_id)
}

fn insert_testbed(
    conn: &mut DbConnection,
    project_id: i32,
    name: &str,
    archived: Option<DateTime>,
) -> i32 {
    diesel::insert_into(schema::testbed::table)
        .values((
            schema::testbed::uuid.eq(TestbedUuid::new()),
            schema::testbed::project_id.eq(project_id),
            schema::testbed::name.eq(name),
            schema::testbed::slug.eq(name),
            schema::testbed::created.eq(seconds(0)),
            schema::testbed::modified.eq(seconds(0)),
            schema::testbed::archived.eq(archived),
        ))
        .execute(&mut *conn)
        .expect("Failed to insert a testbed");
    last_id(conn)
}

fn insert_measure(
    conn: &mut DbConnection,
    project_id: i32,
    name: &str,
    archived: Option<DateTime>,
) -> i32 {
    diesel::insert_into(schema::measure::table)
        .values((
            schema::measure::uuid.eq(MeasureUuid::new()),
            schema::measure::project_id.eq(project_id),
            schema::measure::name.eq(name),
            schema::measure::slug.eq(name),
            schema::measure::units.eq("ns"),
            schema::measure::created.eq(seconds(0)),
            schema::measure::modified.eq(seconds(0)),
            schema::measure::archived.eq(archived),
        ))
        .execute(&mut *conn)
        .expect("Failed to insert a measure");
    last_id(conn)
}

fn insert_threshold(
    conn: &mut DbConnection,
    project_id: i32,
    (branch_id, head_id): (i32, i32),
    testbed_id: i32,
    measure_id: i32,
) -> SeededThreshold {
    diesel::insert_into(schema::threshold::table)
        .values((
            schema::threshold::uuid.eq(ThresholdUuid::new()),
            schema::threshold::project_id.eq(project_id),
            schema::threshold::branch_id.eq(branch_id),
            schema::threshold::testbed_id.eq(testbed_id),
            schema::threshold::measure_id.eq(measure_id),
            schema::threshold::created.eq(seconds(0)),
            schema::threshold::modified.eq(seconds(0)),
        ))
        .execute(&mut *conn)
        .expect("Failed to insert a threshold");
    let id = last_id(conn);
    SeededThreshold {
        id,
        model: insert_model(conn, id),
        head: head_id,
        testbed: testbed_id,
        measure: measure_id,
    }
}

fn insert_model(conn: &mut DbConnection, threshold_id: i32) -> i32 {
    diesel::insert_into(schema::model::table)
        .values((
            schema::model::uuid.eq(ModelUuid::new()),
            schema::model::threshold_id.eq(threshold_id),
            schema::model::test.eq(0),
            schema::model::upper_boundary.eq(Some(0.99)),
            schema::model::created.eq(seconds(0)),
        ))
        .execute(&mut *conn)
        .expect("Failed to insert a model");
    let model_id = last_id(conn);
    diesel::update(schema::threshold::table.filter(schema::threshold::id.eq(threshold_id)))
        .set(schema::threshold::model_id.eq(model_id))
        .execute(&mut *conn)
        .expect("Failed to set the threshold's model");
    model_id
}

fn last_id(conn: &mut DbConnection) -> i32 {
    diesel::select(last_insert_rowid())
        .get_result(conn)
        .expect("Failed to get the last inserted ID")
}

fn seconds(offset: i64) -> DateTime {
    DateTime::try_from(base_timestamp().timestamp() + offset).expect("Invalid timestamp")
}

async fn get_alerts(
    server: &TestServer,
    user: &TestUser,
    slug: &str,
    query: &str,
) -> (Vec<AlertUuid>, u64) {
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{slug}/alerts{query}")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK, "{slug}{query}");
    let total = total_count(resp.headers());
    let alerts: JsonAlerts = resp.json().await.expect("Failed to parse response");
    (
        alerts.0.into_iter().map(|alert| alert.uuid).collect(),
        total,
    )
}

fn total_count(headers: &http::HeaderMap) -> u64 {
    headers
        .get("x-total-count")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse().ok())
        .expect("No total count")
}

async fn capture(server: &TestServer, fixtures: &[&Fixture]) -> Vec<(String, u64)> {
    let mut responses = Vec::new();
    for fixture in fixtures {
        for query in ["", "?archived=true"] {
            let resp = server
                .client
                .get(server.api_url(&format!("/v0/projects/{}/alerts{query}", fixture.slug)))
                .header(
                    bencher_json::AUTHORIZATION,
                    bencher_json::bearer_header(&fixture.user.token),
                )
                .send()
                .await
                .expect("Request failed");
            assert_eq!(resp.status(), StatusCode::OK, "{}{query}", fixture.slug);
            let total = total_count(resp.headers());
            responses.push((resp.text().await.expect("Failed to read response"), total));
        }
    }
    responses
}

/// Revert every migration down to and including the alert project migration.
fn revert_migration(conn: &mut DbConnection) {
    const ALERT_PROJECT_MIGRATION: &str = "20261001120000";

    conn.batch_execute("PRAGMA foreign_keys = OFF")
        .expect("Failed to disable foreign keys");
    loop {
        let version = conn
            .revert_last_migration(MIGRATIONS)
            .expect("Failed to revert a migration");
        if version.to_string() == ALERT_PROJECT_MIGRATION {
            break;
        }
    }
    conn.batch_execute("PRAGMA foreign_keys = ON")
        .expect("Failed to enable foreign keys");
}

fn apply_migration(conn: &mut DbConnection) {
    conn.batch_execute("PRAGMA foreign_keys = OFF")
        .expect("Failed to disable foreign keys");
    conn.run_pending_migrations(MIGRATIONS)
        .expect("Failed to apply the migrations");
    conn.batch_execute("PRAGMA foreign_keys = ON")
        .expect("Failed to enable foreign keys");
}

async fn seed_alerting_project(server: &TestServer, label: &str) -> (TestUser, String, AlertUuid) {
    let user = server
        .signup("Test User", &format!("{label}@example.com"))
        .await;
    let org = server.create_org(&user, &format!("Org {label}")).await;
    let project = server
        .create_project(&user, &org, &format!("Project {label}"))
        .await;
    let slug = project.slug.to_string();

    let post = async |path: String, body: serde_json::Value| {
        server
            .client
            .post(server.api_url(&path))
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(&user.token),
            )
            .json(&body)
            .send()
            .await
            .expect("Request failed")
    };
    for (resource, body) in [
        (
            "branches",
            serde_json::json!({ "name": "own-branch", "slug": "own-branch" }),
        ),
        (
            "testbeds",
            serde_json::json!({ "name": "own-testbed", "slug": "own-testbed" }),
        ),
        (
            "measures",
            serde_json::json!({ "name": "latency", "slug": "latency", "units": "ns" }),
        ),
    ] {
        let resp = post(format!("/v0/projects/{slug}/{resource}"), body).await;
        assert_eq!(resp.status(), StatusCode::CREATED, "POST {resource}");
    }
    let resp = post(
        format!("/v0/projects/{slug}/thresholds"),
        serde_json::json!({
            "branch": "own-branch",
            "testbed": "own-testbed",
            "measure": "latency",
            "test": "t_test",
            "min_sample_size": 2,
            "max_sample_size": 64,
            "upper_boundary": 0.98,
        }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED, "POST thresholds");

    for (day, value) in [10.0, 10.1, 9.9, 10.0, 10.2, 1000.0]
        .into_iter()
        .enumerate()
    {
        let results = serde_json::json!({ "own-benchmark": { "latency": { "value": value } } });
        let body = serde_json::json!({
            "branch": "own-branch",
            "testbed": "own-testbed",
            "start_time": format!("2024-01-{:02}T00:00:00Z", day + 1),
            "end_time": format!("2024-01-{:02}T00:01:00Z", day + 1),
            "results": [serde_json::to_string(&results).expect("Failed to serialize results")],
        });
        let resp = post(format!("/v0/projects/{slug}/reports"), body).await;
        assert_eq!(resp.status(), StatusCode::CREATED, "POST report {day}");
    }

    let (alerts, total) = get_alerts(server, &user, &slug, "").await;
    assert_eq!(total, 1, "the outlier raised one alert");
    let alert = alerts.into_iter().next().expect("No alert");
    (user, slug, alert)
}
