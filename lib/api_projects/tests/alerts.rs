#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args,
    reason = "integration test file"
)]
//! Integration tests for project alert endpoints.

use bencher_api_tests::{
    TestServer, TestUser,
    helpers::{base_timestamp, create_empty_variant, get_project_id, grant_project_role},
};
use bencher_json::{
    AlertUuid, BenchmarkUuid, BoundaryUuid, BranchUuid, DateTime, HeadUuid, JsonAlerts,
    JsonProjectKeyCreated, MeasureUuid, MetricName, MetricUuid, ModelUuid, ReportBenchmarkUuid,
    ReportUuid, TestbedUuid, ThresholdUuid, VersionUuid,
    project::{
        alert::{AlertStatus, JsonUpdatedAlerts, MAX_UPDATE_ALERTS},
        boundary::BoundaryLimit,
    },
};
use bencher_rbac::project::Role;
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
    let base = insert_base(&mut conn, project_id);
    let version_id = base.version;
    let live_branch = insert_branch(&mut conn, project_id, version_id, "live", None);
    let archived_branch = insert_branch(&mut conn, project_id, version_id, "gone", archived);
    let live_testbed = insert_testbed(&mut conn, project_id, "live", None);
    let archived_testbed = insert_testbed(&mut conn, project_id, "gone", archived);
    let live_measure = insert_measure(&mut conn, project_id, "live", None);
    let archived_measure = insert_measure(&mut conn, project_id, "gone", archived);

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
        boundaries.push(insert_boundary(
            &mut conn,
            &base,
            threshold,
            model_id,
            seed.created,
        ));
    }

    Fixture {
        user,
        slug,
        project_id,
        boundaries,
    }
}

/// What every alert of a seeded project shares: its version, benchmark, and variant.
struct SeededBase {
    project: i32,
    version: i32,
    benchmark: i32,
    variant: i32,
}

fn insert_base(conn: &mut DbConnection, project_id: i32) -> SeededBase {
    diesel::insert_into(schema::version::table)
        .values((
            schema::version::uuid.eq(VersionUuid::new()),
            schema::version::project_id.eq(project_id),
            schema::version::number.eq(1),
        ))
        .execute(&mut *conn)
        .expect("Failed to insert the version");
    let version_id = last_id(conn);
    diesel::insert_into(schema::benchmark::table)
        .values((
            schema::benchmark::uuid.eq(BenchmarkUuid::new()),
            schema::benchmark::project_id.eq(project_id),
            schema::benchmark::name.eq("bench"),
            schema::benchmark::slug.eq("bench"),
            schema::benchmark::created.eq(seconds(0)),
            schema::benchmark::modified.eq(seconds(0)),
        ))
        .execute(&mut *conn)
        .expect("Failed to insert the benchmark");
    let benchmark_id = last_id(conn);
    let variant_id = create_empty_variant(conn, benchmark_id);
    SeededBase {
        project: project_id,
        version: version_id,
        benchmark: benchmark_id,
        variant: variant_id,
    }
}

/// A report created `created` seconds after the base timestamp, with one metric that the
/// threshold checked against the model.
///
/// The report starts one second and ends two seconds after it was created, so a window over
/// the wrong one of the three selects other alerts.
fn insert_boundary(
    conn: &mut DbConnection,
    base: &SeededBase,
    threshold: SeededThreshold,
    model_id: i32,
    created: i64,
) -> SeededBoundary {
    diesel::insert_into(schema::report::table)
        .values((
            schema::report::uuid.eq(ReportUuid::new()),
            schema::report::project_id.eq(base.project),
            schema::report::head_id.eq(threshold.head),
            schema::report::version_id.eq(base.version),
            schema::report::testbed_id.eq(threshold.testbed),
            schema::report::adapter.eq(0),
            schema::report::start_time.eq(seconds(created + 1)),
            schema::report::end_time.eq(seconds(created + 2)),
            schema::report::created.eq(seconds(created)),
        ))
        .execute(&mut *conn)
        .expect("Failed to insert a report");
    let report_id = last_id(conn);
    diesel::insert_into(schema::report_benchmark::table)
        .values((
            schema::report_benchmark::uuid.eq(ReportBenchmarkUuid::new()),
            schema::report_benchmark::report_id.eq(report_id),
            schema::report_benchmark::iteration.eq(0),
            schema::report_benchmark::benchmark_id.eq(base.benchmark),
            schema::report_benchmark::variant_id.eq(base.variant),
        ))
        .execute(&mut *conn)
        .expect("Failed to insert a report benchmark");
    let report_benchmark_id = last_id(conn);
    diesel::insert_into(schema::metric::table)
        .values((
            schema::metric::uuid.eq(MetricUuid::new()),
            schema::metric::report_benchmark_id.eq(report_benchmark_id),
            schema::metric::measure_id.eq(threshold.measure),
            schema::metric::name.eq("offset"),
            schema::metric::value.eq(1.0),
        ))
        .execute(&mut *conn)
        .expect("Failed to insert the offset metric");
    diesel::insert_into(schema::metric::table)
        .values((
            schema::metric::uuid.eq(MetricUuid::new()),
            schema::metric::report_benchmark_id.eq(report_benchmark_id),
            schema::metric::measure_id.eq(threshold.measure),
            schema::metric::name.eq(MetricName::value()),
            schema::metric::value.eq(1000.0),
        ))
        .execute(&mut *conn)
        .expect("Failed to insert a metric");
    let metric_id = last_id(conn);
    diesel::insert_into(schema::boundary::table)
        .values((
            schema::boundary::uuid.eq(BoundaryUuid::new()),
            schema::boundary::metric_id.eq(metric_id),
            schema::boundary::threshold_id.eq(threshold.id),
            schema::boundary::model_id.eq(model_id),
            schema::boundary::baseline.eq(Some(1.0)),
            schema::boundary::upper_limit.eq(Some(100.0)),
        ))
        .execute(&mut *conn)
        .expect("Failed to insert a boundary");
    SeededBoundary {
        boundary_id: last_id(conn),
        threshold_id: threshold.id,
    }
}

fn insert_alerts(server: &TestServer, fixture: &Fixture) -> Vec<AlertUuid> {
    let mut conn = server.db_conn();
    SEEDS
        .iter()
        .zip(&fixture.boundaries)
        .map(|(seed, boundary)| {
            insert_alert(
                &mut conn,
                fixture.project_id,
                boundary,
                seed.status,
                seed.modified,
            )
        })
        .collect()
}

fn insert_alert(
    conn: &mut DbConnection,
    project_id: i32,
    boundary: &SeededBoundary,
    status: AlertStatus,
    modified: i64,
) -> AlertUuid {
    let uuid = AlertUuid::new();
    diesel::insert_into(schema::alert::table)
        .values((
            schema::alert::uuid.eq(&uuid),
            schema::alert::project_id.eq(project_id),
            schema::alert::threshold_id.eq(boundary.threshold_id),
            schema::alert::boundary_id.eq(boundary.boundary_id),
            schema::alert::boundary_limit.eq(BoundaryLimit::Upper),
            schema::alert::status.eq(status),
            schema::alert::modified.eq(seconds(modified)),
        ))
        .execute(conn)
        .expect("Failed to insert an alert");
    uuid
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

// PATCH /v0/projects/{project}/console/alerts - the listed alerts of the project change status and
// are stamped as modified now, while a silenced alert, an alert already in the new status, and
// another project's alert are left as they are
#[tokio::test]
async fn console_alerts_change_the_listed_alerts() {
    let server = TestServer::new_at(seconds(100)).await;
    let a = seed_project(&server, "bulklista").await;
    let b = seed_project(&server, "bulklistb").await;
    let a_alerts = insert_alerts(&server, &a);
    let b_alerts = insert_alerts(&server, &b);

    // Active, dismissed, silenced, active on an archived branch, and the other project's active.
    let listed = [
        a_alerts[0],
        a_alerts[3],
        a_alerts[5],
        a_alerts[6],
        b_alerts[0],
    ];
    assert_eq!(
        patch_alerts(
            &server,
            &a.user.token,
            &a.slug,
            serde_json::json!({ "status": "dismissed", "alerts": listed }),
        )
        .await,
        (StatusCode::OK, Some(2))
    );
    assert_eq!(
        alert_states(&server, &listed),
        vec![
            (AlertStatus::Dismissed, seconds(100)),
            (AlertStatus::Dismissed, seconds(5)),
            (AlertStatus::Silenced, seconds(6)),
            (AlertStatus::Dismissed, seconds(100)),
            (AlertStatus::Active, seconds(3)),
        ]
    );
}

// PATCH /v0/projects/{project}/console/alerts - a filter changes every alert of the project that
// matches its current status and its created window, inclusive at both ends, and never an alert
// on an archived branch, testbed, or measure
#[tokio::test]
async fn console_alerts_change_every_alert_the_filter_matches() {
    use AlertStatus::{Active, Dismissed, Silenced};

    let server = TestServer::new().await;
    let a = seed_project(&server, "bulkfiltera").await;
    let b = seed_project(&server, "bulkfilterb").await;
    let a_alerts = insert_alerts(&server, &a);
    let b_alerts = insert_alerts(&server, &b);
    let patch =
        async |body: serde_json::Value| patch_alerts(&server, &a.user.token, &a.slug, body).await;
    let statuses = |alerts: &[AlertUuid]| -> Vec<AlertStatus> {
        alert_states(&server, alerts)
            .into_iter()
            .map(|(status, _)| status)
            .collect()
    };
    let seeded = vec![
        Active, Active, Active, Dismissed, Dismissed, Silenced, Active, Dismissed, Active,
    ];

    assert_eq!(
        patch(serde_json::json!({ "status": "dismissed", "filter": { "status": "dismissed" } }))
            .await,
        (StatusCode::OK, Some(0))
    );
    assert_eq!(statuses(&a_alerts), seeded);

    assert_eq!(
        patch(serde_json::json!({ "status": "dismissed", "filter": { "status": "active" } })).await,
        (StatusCode::OK, Some(3))
    );
    assert_eq!(
        statuses(&a_alerts),
        vec![
            Dismissed, Dismissed, Dismissed, Dismissed, Dismissed, Silenced, Active, Dismissed,
            Active,
        ]
    );
    assert_eq!(statuses(&b_alerts), seeded);

    assert_eq!(
        patch(serde_json::json!({
            "status": "active",
            "filter": {
                "start_time": seconds(3).timestamp_millis(),
                "end_time": seconds(5).timestamp_millis(),
            },
        }))
        .await,
        (StatusCode::OK, Some(2))
    );
    assert_eq!(
        statuses(&a_alerts),
        vec![
            Dismissed, Dismissed, Active, Active, Dismissed, Silenced, Active, Dismissed, Active,
        ]
    );

    // Either bound of the window narrows the filter on its own.
    assert_eq!(
        patch(serde_json::json!({
            "status": "active",
            "filter": { "end_time": seconds(1).timestamp_millis() },
        }))
        .await,
        (StatusCode::OK, Some(1))
    );
    assert_eq!(
        patch(serde_json::json!({
            "status": "dismissed",
            "filter": { "start_time": seconds(5).timestamp_millis() },
        }))
        .await,
        (StatusCode::OK, Some(1))
    );
    assert_eq!(
        statuses(&a_alerts),
        vec![
            Dismissed, Active, Active, Dismissed, Dismissed, Silenced, Active, Dismissed, Active,
        ]
    );
}

// PATCH /v0/projects/{project}/console/alerts - each dimension of a filter narrows it to the
// alerts raised on one of the values it lists
#[tokio::test]
async fn console_alerts_filter_by_dimension() {
    use AlertStatus::{Active, Dismissed};

    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "bulkdimension@example.com")
        .await;
    let org = server.create_org(&user, "Org bulkdimension").await;
    let project = server
        .create_project(&user, &org, "Project bulkdimension")
        .await;
    let slug = project.slug.to_string();
    let project_id = get_project_id(&server, &slug);

    let mut conn = server.db_conn();
    let base = insert_base(&mut conn, project_id);
    let branches = [
        insert_branch(&mut conn, project_id, base.version, "one", None),
        insert_branch(&mut conn, project_id, base.version, "two", None),
    ];
    let testbeds = [
        insert_testbed(&mut conn, project_id, "one", None),
        insert_testbed(&mut conn, project_id, "two", None),
    ];
    let measures = [
        insert_measure(&mut conn, project_id, "one", None),
        insert_measure(&mut conn, project_id, "two", None),
    ];
    // Each threshold after the first differs from it in one dimension.
    let thresholds = [
        (branches[0], testbeds[0], measures[0]),
        (branches[1], testbeds[0], measures[0]),
        (branches[0], testbeds[1], measures[0]),
        (branches[0], testbeds[0], measures[1]),
    ]
    .map(|(branch, testbed, measure)| {
        insert_threshold(&mut conn, project_id, branch, testbed, measure)
    });
    let alerts = thresholds.map(|threshold| {
        let boundary = insert_boundary(&mut conn, &base, threshold, threshold.model, 1);
        insert_alert(&mut conn, project_id, &boundary, Active, 1)
    });
    let uuid = |table: &str, id: i32| -> String {
        #[derive(diesel::QueryableByName)]
        struct Row {
            #[diesel(sql_type = diesel::sql_types::Text)]
            uuid: String,
        }
        diesel::sql_query(format!("SELECT uuid FROM {table} WHERE id = ?"))
            .bind::<diesel::sql_types::Integer, _>(id)
            .get_result::<Row>(&mut server.db_conn())
            .expect("Failed to get a UUID")
            .uuid
    };

    for (dimension, value, changed) in [
        ("branches", uuid("branch", branches[1].0), 1),
        ("testbeds", uuid("testbed", testbeds[1]), 2),
        ("measures", uuid("measure", measures[1]), 3),
        ("thresholds", uuid("threshold", thresholds[0].id), 0),
    ] {
        assert_eq!(
            patch_alerts(
                &server,
                &user.token,
                &slug,
                serde_json::json!({ "status": "dismissed", "filter": { dimension: [value] } }),
            )
            .await,
            (StatusCode::OK, Some(1)),
            "{dimension}"
        );
        let mut expected = [Active; 4];
        expected[changed] = Dismissed;
        let statuses: Vec<_> = alert_states(&server, &alerts)
            .into_iter()
            .map(|(status, _)| status)
            .collect();
        assert_eq!(statuses, expected, "{dimension}");

        assert_eq!(
            patch_alerts(
                &server,
                &user.token,
                &slug,
                serde_json::json!({ "status": "active", "alerts": [alerts[changed]] }),
            )
            .await,
            (StatusCode::OK, Some(1)),
            "{dimension}"
        );
    }

    let both = [uuid("branch", branches[0].0), uuid("branch", branches[1].0)];
    assert_eq!(
        patch_alerts(
            &server,
            &user.token,
            &slug,
            serde_json::json!({ "status": "dismissed", "filter": { "branches": both } }),
        )
        .await,
        (StatusCode::OK, Some(4))
    );
}

// PATCH /v0/projects/{project}/console/alerts - a request selects its alerts by exactly one of a
// list and a filter, and each list holds at most 255 entries
#[tokio::test]
async fn console_alerts_refuse_an_unclear_selection() {
    let server = TestServer::new().await;
    let a = seed_project(&server, "bulkrefuse").await;
    let uuids = |count: usize| -> Vec<AlertUuid> {
        std::iter::repeat_with(AlertUuid::new).take(count).collect()
    };

    let mut requests = vec![
        (
            serde_json::json!({ "status": "dismissed" }),
            (StatusCode::BAD_REQUEST, None),
        ),
        (
            serde_json::json!({ "status": "dismissed", "alerts": [], "filter": {} }),
            (StatusCode::BAD_REQUEST, None),
        ),
        (
            serde_json::json!({ "status": "dismissed", "alerts": uuids(MAX_UPDATE_ALERTS) }),
            (StatusCode::OK, Some(0)),
        ),
        (
            serde_json::json!({ "status": "dismissed", "alerts": uuids(MAX_UPDATE_ALERTS + 1) }),
            (StatusCode::BAD_REQUEST, None),
        ),
    ];
    for dimension in ["branches", "testbeds", "measures", "thresholds"] {
        requests.push((
            serde_json::json!({
                "status": "dismissed",
                "filter": { dimension: uuids(MAX_UPDATE_ALERTS) },
            }),
            (StatusCode::OK, Some(0)),
        ));
        requests.push((
            serde_json::json!({
                "status": "dismissed",
                "filter": { dimension: uuids(MAX_UPDATE_ALERTS + 1) },
            }),
            (StatusCode::BAD_REQUEST, None),
        ));
    }
    for (body, expected) in requests {
        assert_eq!(
            patch_alerts(&server, &a.user.token, &a.slug, body.clone()).await,
            expected,
            "{body}"
        );
    }
}

// PATCH /v0/projects/{project}/console/alerts - changing status takes edit permission, which a
// Developer holds and a Viewer does not, and a project key changes status as it does for one alert
#[tokio::test]
async fn console_alerts_take_edit_permission() {
    let server = TestServer::new().await;
    let a = seed_project(&server, "bulkauth").await;
    let a_alerts = insert_alerts(&server, &a);
    let viewer = server
        .signup("Test User", "bulkauthviewer@example.com")
        .await;
    let developer = server
        .signup("Test User", "bulkauthdeveloper@example.com")
        .await;
    grant_project_role(&server, &viewer, &a.slug, Role::Viewer);
    grant_project_role(&server, &developer, &a.slug, Role::Developer);
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/keys", a.slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&a.user.token),
        )
        .json(&serde_json::json!({ "name": "bulk-key" }))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let key: JsonProjectKeyCreated = resp.json().await.expect("Failed to parse response");

    let dismiss =
        |alert: AlertUuid| serde_json::json!({ "status": "dismissed", "alerts": [alert] });
    assert_eq!(
        patch_alerts(&server, &viewer.token, &a.slug, dismiss(a_alerts[0])).await,
        (StatusCode::FORBIDDEN, None)
    );
    assert_eq!(
        patch_alerts(&server, &developer.token, &a.slug, dismiss(a_alerts[0])).await,
        (StatusCode::OK, Some(1))
    );
    assert_eq!(
        patch_alerts(&server, key.key.as_ref(), &a.slug, dismiss(a_alerts[1])).await,
        (StatusCode::OK, Some(1))
    );
}

async fn patch_alerts(
    server: &TestServer,
    token: &str,
    slug: &str,
    body: serde_json::Value,
) -> (StatusCode, Option<u32>) {
    let resp = server
        .client
        .patch(server.api_url(&format!("/v0/projects/{slug}/console/alerts")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    let status = resp.status();
    if status != StatusCode::OK {
        return (status, None);
    }
    let updated: JsonUpdatedAlerts = resp.json().await.expect("Failed to parse response");
    (status, Some(updated.changed))
}

/// The status and modified time of each alert, in the order given.
fn alert_states(server: &TestServer, alerts: &[AlertUuid]) -> Vec<(AlertStatus, DateTime)> {
    let mut conn = server.db_conn();
    alerts
        .iter()
        .map(|alert| {
            schema::alert::table
                .filter(schema::alert::uuid.eq(alert))
                .select((schema::alert::status, schema::alert::modified))
                .first(&mut conn)
                .expect("Failed to get an alert")
        })
        .collect()
}
