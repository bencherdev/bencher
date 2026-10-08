#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::indexing_slicing,
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
        console::JsonConsoleAlerts,
    },
};
use bencher_rbac::project::Role;
use bencher_schema::{MIGRATIONS, context::DbConnection, macros::sql::last_insert_rowid, schema};
use diesel::{
    ExpressionMethods as _, JoinOnDsl as _, QueryDsl as _, RunQueryDsl as _,
    connection::SimpleConnection as _,
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

/// A report whose one benchmark raised `count` active alerts on the threshold, one per metric.
fn insert_report_alerts(
    conn: &mut DbConnection,
    base: &SeededBase,
    threshold: SeededThreshold,
    count: usize,
) -> ReportUuid {
    let report = ReportUuid::new();
    diesel::insert_into(schema::report::table)
        .values((
            schema::report::uuid.eq(report),
            schema::report::project_id.eq(base.project),
            schema::report::head_id.eq(threshold.head),
            schema::report::version_id.eq(base.version),
            schema::report::testbed_id.eq(threshold.testbed),
            schema::report::adapter.eq(0),
            schema::report::start_time.eq(seconds(2)),
            schema::report::end_time.eq(seconds(3)),
            schema::report::created.eq(seconds(4)),
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
    let metrics = (0..count)
        .map(|index| {
            (
                schema::metric::uuid.eq(MetricUuid::new()),
                schema::metric::report_benchmark_id.eq(report_benchmark_id),
                schema::metric::measure_id.eq(threshold.measure),
                schema::metric::name.eq(format!("metric{index}")),
                schema::metric::value.eq(1000.0),
            )
        })
        .collect::<Vec<_>>();
    diesel::insert_into(schema::metric::table)
        .values(metrics)
        .execute(&mut *conn)
        .expect("Failed to insert the metrics");
    let metric_ids = schema::metric::table
        .filter(schema::metric::report_benchmark_id.eq(report_benchmark_id))
        .select(schema::metric::id)
        .load::<i32>(&mut *conn)
        .expect("Failed to read the metrics");
    let boundaries = metric_ids
        .iter()
        .map(|metric_id| {
            (
                schema::boundary::uuid.eq(BoundaryUuid::new()),
                schema::boundary::metric_id.eq(*metric_id),
                schema::boundary::threshold_id.eq(threshold.id),
                schema::boundary::model_id.eq(threshold.model),
                schema::boundary::baseline.eq(Some(1.0)),
                schema::boundary::upper_limit.eq(Some(100.0)),
            )
        })
        .collect::<Vec<_>>();
    diesel::insert_into(schema::boundary::table)
        .values(boundaries)
        .execute(&mut *conn)
        .expect("Failed to insert the boundaries");
    let boundary_ids = schema::boundary::table
        .filter(schema::boundary::metric_id.eq_any(&metric_ids))
        .select(schema::boundary::id)
        .load::<i32>(&mut *conn)
        .expect("Failed to read the boundaries");
    let alerts = boundary_ids
        .iter()
        .map(|boundary_id| {
            (
                schema::alert::uuid.eq(AlertUuid::new()),
                schema::alert::project_id.eq(base.project),
                schema::alert::threshold_id.eq(threshold.id),
                schema::alert::boundary_id.eq(*boundary_id),
                schema::alert::boundary_limit.eq(BoundaryLimit::Upper),
                schema::alert::status.eq(AlertStatus::Active),
                schema::alert::modified.eq(seconds(4)),
            )
        })
        .collect::<Vec<_>>();
    diesel::insert_into(schema::alert::table)
        .values(alerts)
        .execute(conn)
        .expect("Failed to insert the alerts");
    report
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

// PATCH and GET /v0/projects/{project}/console/alerts - each dimension of a filter narrows Dismiss
// all and the list alike to the alerts raised on one of the values it lists
// Kills: a list that ignores one of the dimension lists.
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
        let query = format!("?{dimension}={value}");
        let listed = read_console_alerts(&server, &user.token, &slug, &query).await;
        assert_eq!(listed_alerts(&listed), [alerts[changed]], "{dimension}");

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
    for dimension in ["branches", "testbeds", "measures", "thresholds", "reports"] {
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

// GET /v0/projects/{project}/console/alerts - the list counts and names exactly the alerts that
// Dismiss all then changes, for the same filters and created window, with millisecond bounds
// inside a second
// Kills: a list window compared in milliseconds or with an exclusive bound, a list that keeps an
// alert on an archived branch, testbed, or measure, and a dimension list read as a single value.
#[tokio::test]
async fn console_alerts_list_what_dismiss_all_changes() {
    let server = TestServer::new().await;
    let cases: [FilterCase; 7] = [
        ("everything", |_| serde_json::json!({}), 3),
        (
            "inside one second",
            |_| {
                serde_json::json!({
                    "start_time": seconds(2).timestamp_millis() + 500,
                    "end_time": seconds(2).timestamp_millis() + 999,
                })
            },
            1,
        ),
        (
            "from late in a second",
            |_| serde_json::json!({ "start_time": seconds(1).timestamp_millis() + 999 }),
            3,
        ),
        (
            "until early in a second",
            |_| serde_json::json!({ "end_time": seconds(1).timestamp_millis() + 1 }),
            1,
        ),
        (
            "live and archived branches",
            |uuids| serde_json::json!({ "branches": [uuids.live_branch, uuids.gone_branch] }),
            3,
        ),
        (
            "an archived branch",
            |uuids| serde_json::json!({ "branches": [uuids.gone_branch] }),
            0,
        ),
        (
            "every dimension",
            |uuids| {
                serde_json::json!({
                    "testbeds": [uuids.live_testbed],
                    "measures": [uuids.live_measure],
                    "thresholds": [uuids.live_threshold],
                })
            },
            3,
        ),
    ];
    let fixture = seed_project(&server, "listbulk").await;
    let alerts = insert_alerts(&server, &fixture);
    let uuids = uuids(&server, &fixture);
    for (case, filter, expected) in cases {
        let filter = filter(&uuids);
        let query = list_query("active", &filter);

        let listed = console_alerts(&server, &fixture, &query).await;
        assert_eq!(listed.total as usize, expected, "{case}");
        let mut named = listed_alerts(&listed);
        assert_eq!(named.len(), expected, "{case}");

        let mut body = filter.clone();
        body["status"] = serde_json::json!("active");
        assert_eq!(
            patch_alerts(
                &server,
                &fixture.user.token,
                &fixture.slug,
                serde_json::json!({ "status": "dismissed", "filter": body }),
            )
            .await,
            (
                StatusCode::OK,
                Some(u32::try_from(expected).expect("a count"))
            ),
            "{case}"
        );
        let mut changed = alerts
            .iter()
            .zip(alert_states(&server, &alerts))
            .zip(SEEDS.iter())
            .filter(|((_, (status, _)), seed)| *status != seed.status)
            .map(|((alert, _), _)| *alert)
            .collect::<Vec<_>>();
        named.sort_unstable();
        changed.sort_unstable();
        assert_eq!(named, changed, "{case}");
        assert_eq!(
            console_alerts(&server, &fixture, &query).await.total,
            0,
            "{case}"
        );
        reset_statuses(&server, &alerts);
    }
}

// GET /v0/projects/{project}/console/alerts - each status lists its alerts newest report first,
// Dismissed with the silenced alerts and All with every alert, while the counts stay the same
// Kills: a Dismissed list without silenced alerts, an All list without them, counts taken from the
// requested status, and groups ordered oldest first.
#[tokio::test]
async fn console_alerts_list_by_status() {
    let server = TestServer::new().await;
    let fixture = seed_project(&server, "liststatus").await;
    let other = seed_project(&server, "liststatusother").await;
    let alerts = insert_alerts(&server, &fixture);
    insert_alerts(&server, &other);
    let seeded = |indices: &[usize]| {
        indices
            .iter()
            .map(|index| alerts[*index])
            .collect::<Vec<_>>()
    };

    for (query, expected) in [
        ("", seeded(&[2, 0, 1])),
        ("?status=active", seeded(&[2, 0, 1])),
        ("?status=dismissed", seeded(&[4, 3, 5])),
        ("?status=all", seeded(&[4, 3, 2, 0, 1, 5])),
    ] {
        let listed = console_alerts(&server, &fixture, query).await;
        assert_eq!(listed_alerts(&listed), expected, "{query}");
        assert_eq!(listed.total as usize, expected.len(), "{query}");
        assert_eq!(
            (
                listed.counts.active,
                listed.counts.dismissed,
                listed.counts.silenced
            ),
            (3, 2, 1),
            "{query}"
        );
        assert!(
            listed.groups.iter().all(|group| group.total == 1),
            "one alert per report"
        );
    }
}

// GET /v0/projects/{project}/console/alerts - pages cut the list in order, reports created in the
// same second newest row first, and a page of no alerts only counts
// Kills: offset off by a page, ties broken oldest report first, a count-only page that reads or
// returns alerts, and a page 0 read as the page before the first.
#[tokio::test]
async fn console_alerts_list_in_pages() {
    let server = TestServer::new().await;
    let fixture = seed_project(&server, "listpages").await;
    let alerts = insert_alerts(&server, &fixture);
    // The first seed's report now shares its second with the third seed's, which was inserted later.
    set_created(&server, &fixture.boundaries[0], seconds(3));
    let seeded = |indices: &[usize]| {
        indices
            .iter()
            .map(|index| alerts[*index])
            .collect::<Vec<_>>()
    };

    let first = console_alerts(&server, &fixture, "?status=all&per_page=4").await;
    assert_eq!(listed_alerts(&first), seeded(&[4, 3, 2, 0]));
    assert_eq!(first.total, 6);
    let zero = console_alerts(&server, &fixture, "?status=all&per_page=4&page=0").await;
    assert_eq!(
        listed_alerts(&zero),
        listed_alerts(&first),
        "page 0 is the first"
    );
    let second = console_alerts(&server, &fixture, "?status=all&per_page=4&page=2").await;
    assert_eq!(listed_alerts(&second), seeded(&[1, 5]));
    assert_eq!(second.total, 6);
    let past = console_alerts(&server, &fixture, "?status=all&per_page=4&page=3").await;
    assert!(past.groups.is_empty());

    let counted = console_alerts(&server, &fixture, "?status=all&per_page=0").await;
    assert_eq!(counted.total, 6);
    assert!(counted.groups.is_empty());
    assert!(counted.reports.is_empty() && counted.benchmarks.is_empty());
}

// GET /v0/projects/{project}/console/alerts - each alert says when it last changed status, or
// when it was raised if it never has
// Kills: an alert's time read from the report that raised it.
#[tokio::test]
async fn console_alerts_list_when_each_alert_changed() {
    let server = TestServer::new_at(seconds(100)).await;
    let fixture = seed_project(&server, "listmodified").await;
    let alerts = insert_alerts(&server, &fixture);
    assert_eq!(
        patch_alerts(
            &server,
            &fixture.user.token,
            &fixture.slug,
            serde_json::json!({ "status": "dismissed", "alerts": [alerts[0]] }),
        )
        .await,
        (StatusCode::OK, Some(1))
    );

    let listed = console_alerts(&server, &fixture, "?status=all").await;
    let mut modified = listed
        .groups
        .iter()
        .flat_map(|group| &group.alerts)
        .map(|alert| {
            (
                alert.line.alert.expect("an alert's line alerted").uuid,
                i64::from(alert.modified),
            )
        })
        .collect::<Vec<_>>();
    modified.sort_unstable();
    let mut expected = alerts
        .iter()
        .zip(&SEEDS)
        .take(6)
        .enumerate()
        .map(|(index, (alert, seed))| {
            let modified = if index == 0 { 100 } else { seed.modified };
            (*alert, seconds(modified).timestamp_millis())
        })
        .collect::<Vec<_>>();
    expected.sort_unstable();
    assert_eq!(modified, expected);
}

// GET /v0/projects/{project}/console/alerts - a branch reported on two heads is listed once under
// each, and each report's group names the head it ran on
// Kills: one branch row for every head of a branch.
#[tokio::test]
async fn console_alerts_list_a_branch_under_each_head() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "listheads@example.com").await;
    let org = server.create_org(&user, "Org listheads").await;
    let project = server
        .create_project(&user, &org, "Project listheads")
        .await;
    let slug = project.slug.to_string();
    let project_id = get_project_id(&server, &slug);

    let mut conn = server.db_conn();
    let base = insert_base(&mut conn, project_id);
    let branch = insert_branch(&mut conn, project_id, base.version, "live", None);
    let testbed = insert_testbed(&mut conn, project_id, "live", None);
    let measure = insert_measure(&mut conn, project_id, "live", None);
    let first = insert_threshold(&mut conn, project_id, branch, testbed, measure);
    let boundary = insert_boundary(&mut conn, &base, first, first.model, 1);
    insert_alert(&mut conn, project_id, &boundary, AlertStatus::Active, 1);
    let replacing = HeadUuid::new();
    diesel::insert_into(schema::head::table)
        .values((
            schema::head::uuid.eq(replacing),
            schema::head::branch_id.eq(branch.0),
            schema::head::created.eq(seconds(2)),
        ))
        .execute(&mut conn)
        .expect("Failed to insert a head");
    let second = SeededThreshold {
        head: last_id(&mut conn),
        ..first
    };
    diesel::insert_into(schema::head_version::table)
        .values((
            schema::head_version::head_id.eq(second.head),
            schema::head_version::version_id.eq(base.version),
        ))
        .execute(&mut conn)
        .expect("Failed to insert a head version");
    let boundary = insert_boundary(&mut conn, &base, second, second.model, 3);
    insert_alert(&mut conn, project_id, &boundary, AlertStatus::Active, 3);
    let replaced = schema::head::table
        .filter(schema::head::id.eq(first.head))
        .select(schema::head::uuid)
        .first::<HeadUuid>(&mut conn)
        .expect("Failed to get the head");
    drop(conn);

    let listed = read_console_alerts(&server, &user.token, &slug, "?status=all").await;
    assert_eq!(
        listed
            .groups
            .iter()
            .map(|group| listed.branches[group.branch as usize].head)
            .collect::<Vec<_>>(),
        [replacing, replaced]
    );
    assert_eq!(listed.branches.len(), 2);
}

// PATCH and GET /v0/projects/{project}/console/alerts - a report filter selects every alert the
// report raised, more than a list of alerts may name, and no other report's
// Kills: a report filter that reaches other reports' alerts or stops at a list's cap.
#[tokio::test]
async fn console_alerts_select_every_alert_a_report_raised() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "bulkreport@example.com").await;
    let org = server.create_org(&user, "Org bulkreport").await;
    let project = server
        .create_project(&user, &org, "Project bulkreport")
        .await;
    let slug = project.slug.to_string();
    let project_id = get_project_id(&server, &slug);

    let mut conn = server.db_conn();
    let base = insert_base(&mut conn, project_id);
    let branch = insert_branch(&mut conn, project_id, base.version, "live", None);
    let testbed = insert_testbed(&mut conn, project_id, "live", None);
    let measure = insert_measure(&mut conn, project_id, "live", None);
    let threshold = insert_threshold(&mut conn, project_id, branch, testbed, measure);
    let boundary = insert_boundary(&mut conn, &base, threshold, threshold.model, 1);
    let other = insert_alert(&mut conn, project_id, &boundary, AlertStatus::Active, 1);
    let report = insert_report_alerts(&mut conn, &base, threshold, MAX_UPDATE_ALERTS + 1);
    drop(conn);

    let query = format!("?reports={report}&per_page=0");
    let listed = read_console_alerts(&server, &user.token, &slug, &query).await;
    assert_eq!(listed.total as usize, MAX_UPDATE_ALERTS + 1);
    assert_eq!(
        patch_alerts(
            &server,
            &user.token,
            &slug,
            serde_json::json!({ "status": "dismissed", "filter": { "reports": [report] } }),
        )
        .await,
        (
            StatusCode::OK,
            Some(u32::try_from(MAX_UPDATE_ALERTS + 1).expect("a count"))
        )
    );
    assert_eq!(
        alert_states(&server, &[other])[0].0,
        AlertStatus::Active,
        "another report's alert"
    );
}

// GET /v0/projects/{project}/console/alerts - a request that cannot be read is refused before
// anything is listed
// Kills: a page past the cap, a window or history size outside its range, a reversed window, and
// an unreadable or overlong list served instead of refused.
#[tokio::test]
async fn console_alerts_refuse_an_unreadable_request() {
    let server = TestServer::new().await;
    let fixture = seed_project(&server, "listrefuse").await;
    insert_alerts(&server, &fixture);
    let overlong = vec![BranchUuid::new().to_string(); MAX_UPDATE_ALERTS + 1].join(",");
    for query in [
        "?per_page=65".to_owned(),
        "?window=0".to_owned(),
        "?window=367".to_owned(),
        "?points=1".to_owned(),
        format!(
            "?start_time={}&end_time={}",
            seconds(2).timestamp_millis(),
            seconds(1).timestamp_millis()
        ),
        "?branches=main".to_owned(),
        "?testbeds=,".to_owned(),
        "?reports=latest".to_owned(),
        format!("?branches={overlong}"),
        format!("?reports={overlong}"),
    ] {
        let (status, text) =
            try_console_alerts(&server, &fixture.user.token, &fixture.slug, &query).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{query}: {text}");
    }
    let (status, text) = try_console_alerts(
        &server,
        &fixture.user.token,
        &fixture.slug,
        "?per_page=64&branches=",
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "an empty list filters nothing: {text}"
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

fn reset_statuses(server: &TestServer, alerts: &[AlertUuid]) {
    let mut conn = server.db_conn();
    for (alert, seed) in alerts.iter().zip(&SEEDS) {
        diesel::update(schema::alert::table.filter(schema::alert::uuid.eq(alert)))
            .set(schema::alert::status.eq(seed.status))
            .execute(&mut conn)
            .expect("Failed to reset an alert");
    }
}

/// A filter, how it reads its UUIDs, and how many active alerts it selects.
type FilterCase = (&'static str, fn(&Uuids) -> serde_json::Value, usize);

/// The UUIDs a seeded project's filters name.
struct Uuids {
    live_branch: BranchUuid,
    gone_branch: BranchUuid,
    live_testbed: TestbedUuid,
    live_measure: MeasureUuid,
    live_threshold: ThresholdUuid,
}

fn uuids(server: &TestServer, fixture: &Fixture) -> Uuids {
    let mut conn = server.db_conn();
    let branch = |conn: &mut DbConnection, name: &str| {
        schema::branch::table
            .filter(schema::branch::project_id.eq(fixture.project_id))
            .filter(schema::branch::name.eq(name))
            .select(schema::branch::uuid)
            .first::<BranchUuid>(conn)
            .expect("Failed to get a branch")
    };
    Uuids {
        live_branch: branch(&mut conn, "live"),
        gone_branch: branch(&mut conn, "gone"),
        live_testbed: schema::testbed::table
            .filter(schema::testbed::project_id.eq(fixture.project_id))
            .filter(schema::testbed::name.eq("live"))
            .select(schema::testbed::uuid)
            .first(&mut conn)
            .expect("Failed to get the testbed"),
        live_measure: schema::measure::table
            .filter(schema::measure::project_id.eq(fixture.project_id))
            .filter(schema::measure::name.eq("live"))
            .select(schema::measure::uuid)
            .first(&mut conn)
            .expect("Failed to get the measure"),
        live_threshold: schema::threshold::table
            .filter(schema::threshold::id.eq(fixture.boundaries[0].threshold_id))
            .select(schema::threshold::uuid)
            .first(&mut conn)
            .expect("Failed to get the threshold"),
    }
}

/// The list's query for a Dismiss all filter's fields.
fn list_query(status: &str, filter: &serde_json::Value) -> String {
    let mut pairs = vec![format!("status={status}"), "per_page=64".to_owned()];
    for (key, value) in filter.as_object().expect("a filter object") {
        let value = if let Some(values) = value.as_array() {
            values
                .iter()
                .map(|value| value.as_str().expect("a UUID").to_owned())
                .collect::<Vec<_>>()
                .join(",")
        } else {
            value.to_string()
        };
        pairs.push(format!("{key}={value}"));
    }
    format!("?{}", pairs.join("&"))
}

fn set_created(server: &TestServer, boundary: &SeededBoundary, created: DateTime) {
    let mut conn = server.db_conn();
    let report_id: i32 = schema::boundary::table
        .inner_join(schema::metric::table.on(schema::metric::id.eq(schema::boundary::metric_id)))
        .inner_join(
            schema::report_benchmark::table
                .on(schema::report_benchmark::id.eq(schema::metric::report_benchmark_id)),
        )
        .filter(schema::boundary::id.eq(boundary.boundary_id))
        .select(schema::report_benchmark::report_id)
        .first(&mut conn)
        .expect("Failed to get the boundary's report");
    diesel::update(schema::report::table.filter(schema::report::id.eq(report_id)))
        .set(schema::report::created.eq(created))
        .execute(&mut conn)
        .expect("Failed to set when the report was created");
}

async fn try_console_alerts(
    server: &TestServer,
    token: &str,
    slug: &str,
    query: &str,
) -> (StatusCode, String) {
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{slug}/console/alerts{query}")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(token),
        )
        .send()
        .await
        .expect("Request failed");
    let status = resp.status();
    (
        status,
        resp.text().await.expect("Failed to read the response"),
    )
}

async fn console_alerts(server: &TestServer, fixture: &Fixture, query: &str) -> JsonConsoleAlerts {
    read_console_alerts(server, &fixture.user.token, &fixture.slug, query).await
}

async fn read_console_alerts(
    server: &TestServer,
    token: &str,
    slug: &str,
    query: &str,
) -> JsonConsoleAlerts {
    let (status, text) = try_console_alerts(server, token, slug, query).await;
    assert_eq!(status, StatusCode::OK, "GET console alerts{query}: {text}");
    serde_json::from_str(&text).expect("Failed to parse the console alerts")
}

/// Every alert on the page, in list order.
fn listed_alerts(alerts: &JsonConsoleAlerts) -> Vec<AlertUuid> {
    alerts
        .groups
        .iter()
        .flat_map(|group| &group.alerts)
        .map(|alert| alert.line.alert.expect("an alert's line alerted").uuid)
        .collect()
}
