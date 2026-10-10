#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::tests_outside_test_module,
    clippy::too_many_lines,
    reason = "integration test file"
)]
//! Integration tests for the console's threshold endpoints.

use bencher_api_tests::{
    TestServer, TestUser,
    helpers::{base_timestamp, create_empty_variant, get_project_id, grant_project_role},
};
use bencher_json::{
    AlertUuid, BenchmarkUuid, BoundaryUuid, BranchUuid, DateTime, HeadUuid, JsonProjectKeyCreated,
    MeasureUuid, MetricName, MetricUuid, ModelTest, ModelUuid, ParameterFilter,
    ReportBenchmarkUuid, ReportUuid, TestbedUuid, ThresholdUuid, VersionUuid,
    project::{
        Visibility,
        alert::AlertStatus,
        boundary::BoundaryLimit,
        threshold::{JsonConsoleThreshold, JsonConsoleThresholds},
    },
};
use bencher_rbac::project::Role;
use bencher_schema::{context::DbConnection, macros::sql::last_insert_rowid, schema};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use http::StatusCode;
use serde_json::{Value, json};

// GET /v0/projects/{project}/console/thresholds - every threshold whose branch, testbed, and
// measure are live, oldest first by creation (not by row), each with its dimensions once in the
// tables, its current model and not a replaced one, no model when it was removed, and the count
// of the alerts it raised and of those active now
#[tokio::test]
async fn console_thresholds_list_each_threshold_with_its_dimensions_and_model() {
    let server = TestServer::new().await;
    let f = seed_project(&server, "thlist").await;
    seed_project(&server, "thlistother").await;

    let (status, body) = get(&server, &f.user.token, &f.slug, "").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({
            "total": 3,
            "thresholds": [
                {
                    "uuid": f.latency.uuid,
                    "branch": 0,
                    "testbed": 0,
                    "measure": 0,
                    "model": {
                        "test": "t_test",
                        "min_sample_size": 5,
                        "max_sample_size": 30,
                        "window": 7_776_000,
                        "upper_boundary": 0.99,
                    },
                    "raised": 4,
                    "active": 2,
                },
                {
                    "uuid": f.cycles.uuid,
                    "branch": 0,
                    "testbed": 0,
                    "measure": 1,
                    "parameters": [{ "input_bytes": 64 }],
                    "metric": "p99",
                    "model": {
                        "test": "percentage",
                        "upper_boundary": 0.01,
                    },
                    "raised": 1,
                    "active": 1,
                },
                {
                    "uuid": f.reset.uuid,
                    "branch": 1,
                    "testbed": 0,
                    "measure": 0,
                    "raised": 1,
                    "active": 0,
                },
            ],
            "branches": [
                { "uuid": f.main, "name": "main", "slug": "main" },
                { "uuid": f.feature, "name": "feature", "slug": "feature", "start_point": "main" },
            ],
            "testbeds": [{ "uuid": f.linux, "name": "linux", "slug": "linux" }],
            "measures": [
                { "uuid": f.latency_measure, "name": "latency", "slug": "latency", "units": "ns" },
                { "uuid": f.cycles_measure, "name": "cycles", "slug": "cycles", "units": "cycles" },
            ],
        })
    );
}

// GET /v0/projects/{project}/console/thresholds - the window counts the alerts whose report was
// created inside it, inclusive at both ends and not by the report's start or end time, whatever
// their status, while the active count ignores the window
#[tokio::test]
async fn console_thresholds_count_the_alerts_raised_inside_the_window() {
    let server = TestServer::new().await;
    let f = seed_project(&server, "thwindow").await;

    for (query, latency, cycles, reset) in [
        (window(Some(2000), Some(3000)), 2, 1, 0),
        (window(Some(2001), Some(2999)), 0, 0, 0),
        (window(Some(3000), None), 2, 0, 1),
        (window(None, Some(1000)), 1, 0, 0),
        (window(Some(1000), Some(5000)), 4, 1, 1),
    ] {
        let (status, body) = get(&server, &f.user.token, &f.slug, &query).await;
        assert_eq!(status, StatusCode::OK, "{query}");
        assert_eq!(
            counts(&body),
            vec![
                (f.latency.uuid.to_string(), latency, 2),
                (f.cycles.uuid.to_string(), cycles, 1),
                (f.reset.uuid.to_string(), reset, 0),
            ],
            "{query}"
        );
    }
}

// GET /v0/projects/{project}/console/thresholds - a window that ends before it starts is refused
#[tokio::test]
async fn console_thresholds_refuse_a_window_that_ends_before_it_starts() {
    let server = TestServer::new().await;
    let f = seed_project(&server, "thbadwindow").await;

    let (status, _) = get(
        &server,
        &f.user.token,
        &f.slug,
        &window(Some(3000), Some(2000)),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// GET /v0/projects/{project}/console/thresholds - each dimension filter alone narrows the list to
// that branch, testbed, or measure, and `archived=true` lists only the thresholds with an
// archived branch, testbed, or measure, each dimension saying when it was archived
#[tokio::test]
async fn console_thresholds_filter_by_dimension_and_archived() {
    let server = TestServer::new().await;
    let f = seed_project(&server, "thfilter").await;

    for (query, expected) in [
        (format!("?branch={}", f.feature), vec![f.reset.uuid]),
        (
            format!("?testbed={}", f.linux),
            vec![f.latency.uuid, f.cycles.uuid, f.reset.uuid],
        ),
        (
            format!("?measure={}", f.cycles_measure),
            vec![f.cycles.uuid],
        ),
        (
            format!("?branch={}&measure={}", f.main, f.latency_measure),
            vec![f.latency.uuid],
        ),
        (
            "?archived=true".to_owned(),
            vec![f.on_gone, f.on_old, f.on_retired],
        ),
        (format!("?archived=true&testbed={}", f.old), vec![f.on_old]),
    ] {
        let (status, body) = get(&server, &f.user.token, &f.slug, &query).await;
        assert_eq!(status, StatusCode::OK, "{query}");
        let list: JsonConsoleThresholds =
            serde_json::from_value(body).expect("Failed to parse response");
        assert_eq!(
            list.thresholds
                .iter()
                .map(|threshold| threshold.uuid)
                .collect::<Vec<_>>(),
            expected,
            "{query}"
        );
        assert_eq!(
            usize::try_from(list.total).expect("Invalid total"),
            expected.len(),
            "{query}"
        );
    }

    let (_, body) = get(&server, &f.user.token, &f.slug, "?archived=true").await;
    assert_eq!(
        archived(&body),
        vec![
            ("branch", "gone".to_owned(), Some(millis(500))),
            ("branch", "main".to_owned(), None),
            ("testbed", "linux".to_owned(), None),
            ("testbed", "old".to_owned(), Some(millis(600))),
            ("measure", "latency".to_owned(), None),
            ("measure", "retired".to_owned(), Some(millis(700))),
        ]
    );
}

// GET /v0/projects/{project}/console/thresholds - pages split the list in order while the total
// counts every match, and a page's tables hold only what its rows refer to
#[tokio::test]
async fn console_thresholds_page_through_the_list() {
    let server = TestServer::new().await;
    let f = seed_project(&server, "thpage").await;

    let (_, first) = get(&server, &f.user.token, &f.slug, "?per_page=2").await;
    let first: JsonConsoleThresholds =
        serde_json::from_value(first).expect("Failed to parse response");
    let (_, second) = get(&server, &f.user.token, &f.slug, "?per_page=2&page=2").await;
    let second: JsonConsoleThresholds =
        serde_json::from_value(second).expect("Failed to parse response");

    assert_eq!((first.total, second.total), (3, 3));
    assert_eq!(
        first
            .thresholds
            .iter()
            .map(|threshold| threshold.uuid)
            .collect::<Vec<_>>(),
        vec![f.latency.uuid, f.cycles.uuid]
    );
    assert_eq!(
        second
            .thresholds
            .iter()
            .map(|threshold| threshold.uuid)
            .collect::<Vec<_>>(),
        vec![f.reset.uuid]
    );
    assert_eq!(
        second
            .branches
            .iter()
            .map(|branch| branch.uuid)
            .collect::<Vec<_>>(),
        vec![f.feature]
    );
    assert_eq!(
        second
            .measures
            .iter()
            .map(|measure| measure.uuid)
            .collect::<Vec<_>>(),
        vec![f.latency_measure]
    );
    assert_eq!(second.thresholds[0].branch, 0);
}

// GET /v0/projects/{project}/console/thresholds/{threshold} - the threshold with its dimensions,
// its current model, and every model it has had newest first, each replaced one saying when
#[tokio::test]
async fn console_threshold_with_its_model_history() {
    let server = TestServer::new().await;
    let f = seed_project(&server, "thone").await;

    let (status, body) = get_one(&server, &f.user.token, &f.slug, f.latency.uuid).await;
    assert_eq!(status, StatusCode::OK);
    let current = json!({
        "uuid": f.latency.models[2],
        "test": "t_test",
        "min_sample_size": 5,
        "max_sample_size": 30,
        "window": 7_776_000,
        "upper_boundary": 0.99,
        "created": millis(20),
    });
    assert_eq!(
        body,
        json!({
            "uuid": f.latency.uuid,
            "branch": { "uuid": f.main, "name": "main", "slug": "main" },
            "testbed": { "uuid": f.linux, "name": "linux", "slug": "linux" },
            "measure": { "uuid": f.latency_measure, "name": "latency", "slug": "latency", "units": "ns" },
            "model": current,
            "models": [
                current,
                {
                    "uuid": f.latency.models[1],
                    "test": "percentage",
                    "upper_boundary": 0.05,
                    "created": millis(20),
                    "replaced": millis(20),
                },
                {
                    "uuid": f.latency.models[0],
                    "test": "z_score",
                    "max_sample_size": 64,
                    "upper_boundary": 0.997,
                    "created": millis(10),
                    "replaced": millis(20),
                },
            ],
            "created": millis(10),
            "modified": millis(20),
        })
    );

    let (status, body) = get_one(&server, &f.user.token, &f.slug, f.reset.uuid).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({
            "uuid": f.reset.uuid,
            "branch": { "uuid": f.feature, "name": "feature", "slug": "feature", "start_point": "main" },
            "testbed": { "uuid": f.linux, "name": "linux", "slug": "linux" },
            "measure": { "uuid": f.latency_measure, "name": "latency", "slug": "latency", "units": "ns" },
            "models": [{
                "uuid": f.reset.models[0],
                "test": "t_test",
                "upper_boundary": 0.99,
                "created": millis(12),
                "replaced": millis(30),
            }],
            "created": millis(12),
            "modified": millis(12),
        })
    );

    let (status, body) = get_one(&server, &f.user.token, &f.slug, f.on_gone).await;
    assert_eq!(status, StatusCode::OK);
    let threshold: JsonConsoleThreshold =
        serde_json::from_value(body).expect("Failed to parse response");
    assert_eq!(
        threshold.branch.archived.map(i64::from),
        Some(millis(500)),
        "an archived threshold still opens"
    );
}

// GET /v0/projects/{project}/console/thresholds/{threshold} - another project's threshold is not
// found through this project
#[tokio::test]
async fn console_threshold_belongs_to_its_project() {
    let server = TestServer::new().await;
    let f = seed_project(&server, "thown").await;
    let other = seed_project(&server, "thownother").await;

    let (status, _) = get_one(&server, &f.user.token, &f.slug, other.latency.uuid).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// GET /v0/projects/{project}/console/thresholds{,/{threshold}} - reading takes a signed-in
// reader or the project's own key, even on a public project, and a private project is not found
// by anyone who may not view it
#[tokio::test]
async fn console_thresholds_take_a_signed_in_reader() {
    use StatusCode as S;

    let server = TestServer::new().await;
    let public = seed_project(&server, "thauth").await;
    let private = seed_project(&server, "thauthprivate").await;
    diesel::update(schema::project::table.filter(schema::project::slug.eq(&private.slug)))
        .set(schema::project::visibility.eq(Visibility::Private))
        .execute(&mut server.db_conn())
        .expect("Failed to make the project private");
    let other = seed_project(&server, "thauthother").await;
    let viewer = server
        .signup("Threshold Viewer", "thauthviewer@example.com")
        .await;
    grant_project_role(&server, &viewer, &public.slug, Role::Viewer);
    grant_project_role(&server, &viewer, &private.slug, Role::Viewer);
    let stranger = server
        .signup("Threshold Stranger", "thauthstranger@example.com")
        .await;
    let public_key = create_key(&server, &public.user, &public.slug).await;
    let private_key = create_key(&server, &private.user, &private.slug).await;
    let other_key = create_key(&server, &other.user, &other.slug).await;

    for (f, own_key, anonymous, strangers, others_key) in [
        (&public, &public_key, S::UNAUTHORIZED, S::OK, S::FORBIDDEN),
        (
            &private,
            &private_key,
            S::NOT_FOUND,
            S::NOT_FOUND,
            S::NOT_FOUND,
        ),
    ] {
        for (token, expected) in [
            (None, anonymous),
            (Some(viewer.token.as_str()), S::OK),
            (Some(stranger.token.as_str()), strangers),
            (Some(own_key.as_str()), S::OK),
            (Some(other_key.as_str()), others_key),
        ] {
            let list = request(
                &server,
                token,
                &format!("/v0/projects/{}/console/thresholds", f.slug),
            )
            .await
            .0;
            let one = request(
                &server,
                token,
                &format!(
                    "/v0/projects/{}/console/thresholds/{}",
                    f.slug, f.latency.uuid
                ),
            )
            .await
            .0;
            assert_eq!((list, one), (expected, expected), "{} {token:?}", f.slug);
        }
    }
}

struct Fixture {
    user: TestUser,
    slug: String,
    main: BranchUuid,
    feature: BranchUuid,
    linux: TestbedUuid,
    old: TestbedUuid,
    latency_measure: MeasureUuid,
    cycles_measure: MeasureUuid,
    latency: SeededThreshold,
    cycles: SeededThreshold,
    reset: SeededThreshold,
    on_gone: ThresholdUuid,
    on_old: ThresholdUuid,
    on_retired: ThresholdUuid,
}

#[derive(Clone)]
struct SeededThreshold {
    id: i32,
    uuid: ThresholdUuid,
    /// Oldest first.
    models: Vec<ModelUuid>,
}

/// A project with three live thresholds and one on each kind of archived dimension:
/// - `latency` on main, linux, latency, whose first two models were replaced by its third
/// - `cycles` on main, linux, cycles, with a parameters filter and a metric name
/// - `reset` on feature, started from main, whose only model was removed
///
/// The rows go in out of creation order, so an order by row is not an order by creation.
/// Alerts: `latency` raised one in each report created at 1,000 (active), 2,000 (dismissed),
/// 3,000 (silenced), and 4,000 (active) seconds, `cycles` one at 2,000 (active), and `reset` one
/// at 5,000 (dismissed).
async fn seed_project(server: &TestServer, label: &str) -> Fixture {
    let user = server
        .signup(
            &format!("Threshold {label}"),
            &format!("{label}@example.com"),
        )
        .await;
    let org = server.create_org(&user, &format!("Org {label}")).await;
    let project = server
        .create_project(&user, &org, &format!("Project {label}"))
        .await;
    let slug = project.slug.to_string();
    let project_id = get_project_id(server, &slug);

    let mut conn = server.db_conn();
    let base = insert_base(&mut conn, project_id);
    let main = insert_branch(&mut conn, &base, "main", None, None);
    let feature = insert_branch(&mut conn, &base, "feature", Some(main.head_version), None);
    let gone = insert_branch(&mut conn, &base, "gone", None, Some(500));
    // A head of `feature` that is not its current one, so only the current head names main.
    diesel::insert_into(schema::head::table)
        .values((
            schema::head::uuid.eq(HeadUuid::new()),
            schema::head::branch_id.eq(feature.id),
            schema::head::start_point_id.eq(Some(gone.head_version)),
            schema::head::created.eq(seconds(0)),
        ))
        .execute(&mut conn)
        .expect("Failed to insert a head that is not the branch's current one");
    let linux = insert_dimension(&mut conn, project_id, Dimension::Testbed, "linux", None);
    let old = insert_dimension(&mut conn, project_id, Dimension::Testbed, "old", Some(600));
    let latency_measure = insert_dimension(
        &mut conn,
        project_id,
        Dimension::Measure("ns"),
        "latency",
        None,
    );
    let cycles_measure = insert_dimension(
        &mut conn,
        project_id,
        Dimension::Measure("cycles"),
        "cycles",
        None,
    );
    let retired = insert_dimension(
        &mut conn,
        project_id,
        Dimension::Measure("ns"),
        "retired",
        Some(700),
    );

    let t_test = Model {
        test: ModelTest::TTest,
        upper_boundary: 0.99,
        ..Model::default()
    };
    let mut reset = insert_threshold(
        &mut conn,
        project_id,
        (&feature, linux, latency_measure),
        None,
        12,
    );
    let removed = insert_model(&mut conn, &reset, 12, Some(30), t_test);
    reset.models.push(removed);
    let mut latency = insert_threshold(
        &mut conn,
        project_id,
        (&main, linux, latency_measure),
        None,
        10,
    );
    let replaced = insert_model(
        &mut conn,
        &latency,
        10,
        Some(20),
        Model {
            test: ModelTest::ZScore,
            max_sample_size: Some(64),
            upper_boundary: 0.997,
            ..Model::default()
        },
    );
    latency.models.push(replaced);
    // Set and replaced inside the second its successor was set in, so only the row order
    // puts the current model first.
    let brief = insert_model(
        &mut conn,
        &latency,
        20,
        Some(20),
        Model {
            test: ModelTest::Percentage,
            upper_boundary: 0.05,
            ..Model::default()
        },
    );
    latency.models.push(brief);
    let current = Model {
        min_sample_size: Some(5),
        max_sample_size: Some(30),
        window: Some(7_776_000),
        ..t_test
    };
    let current = insert_model(&mut conn, &latency, 20, None, current);
    latency.models.push(current);
    set_model(&mut conn, &latency, 20);
    let mut cycles = insert_threshold(
        &mut conn,
        project_id,
        (&main, linux, cycles_measure),
        Some((
            serde_json::from_value(json!([{ "input_bytes": 64 }])).expect("Invalid filter"),
            "p99",
        )),
        11,
    );
    let percentage = Model {
        test: ModelTest::Percentage,
        upper_boundary: 0.01,
        ..Model::default()
    };
    let percentage = insert_model(&mut conn, &cycles, 11, None, percentage);
    cycles.models.push(percentage);
    set_model(&mut conn, &cycles, 11);

    let mut on_archived = Vec::new();
    for (index, dimensions) in [
        (&gone, linux, latency_measure),
        (&main, old, latency_measure),
        (&main, linux, retired),
    ]
    .into_iter()
    .enumerate()
    {
        let created = 13 + i64::try_from(index).expect("Invalid index");
        let threshold = insert_threshold(&mut conn, project_id, dimensions, None, created);
        insert_model(&mut conn, &threshold, created, None, t_test);
        set_model(&mut conn, &threshold, created);
        on_archived.push(threshold.uuid);
    }

    for (threshold, (head, measure), created, status) in [
        (
            &latency,
            (&main, latency_measure),
            1000,
            AlertStatus::Active,
        ),
        (
            &latency,
            (&main, latency_measure),
            2000,
            AlertStatus::Dismissed,
        ),
        (
            &latency,
            (&main, latency_measure),
            3000,
            AlertStatus::Silenced,
        ),
        (
            &latency,
            (&main, latency_measure),
            4000,
            AlertStatus::Active,
        ),
        (&cycles, (&main, cycles_measure), 2000, AlertStatus::Active),
        (
            &reset,
            (&feature, latency_measure),
            5000,
            AlertStatus::Dismissed,
        ),
    ] {
        insert_alert(
            &mut conn,
            &base,
            (threshold, *threshold.models.last().expect("No model")),
            (head.head, linux.0, measure.0),
            created,
            status,
        );
    }

    Fixture {
        user,
        slug,
        main: main.uuid,
        feature: feature.uuid,
        linux: TestbedUuid::from(linux.1),
        old: TestbedUuid::from(old.1),
        latency_measure: MeasureUuid::from(latency_measure.1),
        cycles_measure: MeasureUuid::from(cycles_measure.1),
        latency,
        cycles,
        reset,
        on_gone: on_archived[0],
        on_old: on_archived[1],
        on_retired: on_archived[2],
    }
}

/// What every report of a seeded project shares: its version, benchmark, and variant.
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
    let version = last_id(conn);
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
    let benchmark = last_id(conn);
    let variant = create_empty_variant(conn, benchmark);
    SeededBase {
        project: project_id,
        version,
        benchmark,
        variant,
    }
}

struct SeededBranch {
    id: i32,
    uuid: BranchUuid,
    head: i32,
    head_version: i32,
}

fn insert_branch(
    conn: &mut DbConnection,
    base: &SeededBase,
    name: &str,
    start_point: Option<i32>,
    archived: Option<i64>,
) -> SeededBranch {
    let uuid = BranchUuid::new();
    diesel::insert_into(schema::branch::table)
        .values((
            schema::branch::uuid.eq(uuid),
            schema::branch::project_id.eq(base.project),
            schema::branch::name.eq(name),
            schema::branch::slug.eq(name),
            schema::branch::created.eq(seconds(0)),
            schema::branch::modified.eq(seconds(0)),
            schema::branch::archived.eq(archived.map(seconds)),
        ))
        .execute(&mut *conn)
        .expect("Failed to insert a branch");
    let id = last_id(conn);
    diesel::insert_into(schema::head::table)
        .values((
            schema::head::uuid.eq(HeadUuid::new()),
            schema::head::branch_id.eq(id),
            schema::head::start_point_id.eq(start_point),
            schema::head::created.eq(seconds(0)),
        ))
        .execute(&mut *conn)
        .expect("Failed to insert a head");
    let head = last_id(conn);
    diesel::update(schema::branch::table.filter(schema::branch::id.eq(id)))
        .set(schema::branch::head_id.eq(head))
        .execute(&mut *conn)
        .expect("Failed to point the branch at its head");
    diesel::insert_into(schema::head_version::table)
        .values((
            schema::head_version::head_id.eq(head),
            schema::head_version::version_id.eq(base.version),
        ))
        .execute(&mut *conn)
        .expect("Failed to insert a head version");
    SeededBranch {
        id,
        uuid,
        head,
        head_version: last_id(conn),
    }
}

#[derive(Clone, Copy)]
enum Dimension {
    Testbed,
    /// A measure with its units.
    Measure(&'static str),
}

/// A testbed or measure, as its ID and UUID.
fn insert_dimension(
    conn: &mut DbConnection,
    project_id: i32,
    dimension: Dimension,
    name: &str,
    archived: Option<i64>,
) -> (i32, uuid::Uuid) {
    let uuid = uuid::Uuid::new_v4();
    match dimension {
        Dimension::Testbed => diesel::insert_into(schema::testbed::table)
            .values((
                schema::testbed::uuid.eq(uuid.to_string()),
                schema::testbed::project_id.eq(project_id),
                schema::testbed::name.eq(name),
                schema::testbed::slug.eq(name),
                schema::testbed::created.eq(seconds(0)),
                schema::testbed::modified.eq(seconds(0)),
                schema::testbed::archived.eq(archived.map(seconds)),
            ))
            .execute(&mut *conn),
        Dimension::Measure(units) => diesel::insert_into(schema::measure::table)
            .values((
                schema::measure::uuid.eq(uuid.to_string()),
                schema::measure::project_id.eq(project_id),
                schema::measure::name.eq(name),
                schema::measure::slug.eq(name),
                schema::measure::units.eq(units),
                schema::measure::created.eq(seconds(0)),
                schema::measure::modified.eq(seconds(0)),
                schema::measure::archived.eq(archived.map(seconds)),
            ))
            .execute(&mut *conn),
    }
    .expect("Failed to insert a dimension");
    (last_id(conn), uuid)
}

/// A threshold with no model yet, created and modified at `created` seconds.
fn insert_threshold(
    conn: &mut DbConnection,
    project_id: i32,
    (branch, testbed, measure): (&SeededBranch, (i32, uuid::Uuid), (i32, uuid::Uuid)),
    filter: Option<(ParameterFilter, &str)>,
    created: i64,
) -> SeededThreshold {
    let uuid = ThresholdUuid::new();
    let (parameters, metric) = filter.map_or((None, None), |(parameters, metric)| {
        (Some(parameters), Some(metric.to_owned()))
    });
    diesel::insert_into(schema::threshold::table)
        .values((
            schema::threshold::uuid.eq(uuid),
            schema::threshold::project_id.eq(project_id),
            schema::threshold::branch_id.eq(branch.id),
            schema::threshold::testbed_id.eq(testbed.0),
            schema::threshold::parameters.eq(parameters),
            schema::threshold::measure_id.eq(measure.0),
            schema::threshold::metric.eq(metric),
            schema::threshold::created.eq(seconds(created)),
            schema::threshold::modified.eq(seconds(created)),
        ))
        .execute(&mut *conn)
        .expect("Failed to insert a threshold");
    SeededThreshold {
        id: last_id(conn),
        uuid,
        models: Vec::new(),
    }
}

#[derive(Clone, Copy)]
struct Model {
    test: ModelTest,
    min_sample_size: Option<i64>,
    max_sample_size: Option<i64>,
    window: Option<i64>,
    upper_boundary: f64,
}

impl Default for Model {
    fn default() -> Self {
        Self {
            test: ModelTest::Static,
            min_sample_size: None,
            max_sample_size: None,
            window: None,
            upper_boundary: 0.0,
        }
    }
}

fn insert_model(
    conn: &mut DbConnection,
    threshold: &SeededThreshold,
    created: i64,
    replaced: Option<i64>,
    model: Model,
) -> ModelUuid {
    let uuid = ModelUuid::new();
    diesel::insert_into(schema::model::table)
        .values((
            schema::model::uuid.eq(uuid),
            schema::model::threshold_id.eq(threshold.id),
            schema::model::test.eq(model.test),
            schema::model::min_sample_size.eq(model.min_sample_size),
            schema::model::max_sample_size.eq(model.max_sample_size),
            schema::model::window.eq(model.window),
            schema::model::upper_boundary.eq(Some(model.upper_boundary)),
            schema::model::created.eq(seconds(created)),
            schema::model::replaced.eq(replaced.map(seconds)),
        ))
        .execute(&mut *conn)
        .expect("Failed to insert a model");
    uuid
}

/// Make the threshold's newest model its current one, modified at `modified` seconds.
fn set_model(conn: &mut DbConnection, threshold: &SeededThreshold, modified: i64) {
    let model_id: i32 = schema::model::table
        .filter(schema::model::threshold_id.eq(threshold.id))
        .order(schema::model::id.desc())
        .select(schema::model::id)
        .first(&mut *conn)
        .expect("Failed to get the threshold's model");
    diesel::update(schema::threshold::table.filter(schema::threshold::id.eq(threshold.id)))
        .set((
            schema::threshold::model_id.eq(model_id),
            schema::threshold::modified.eq(seconds(modified)),
        ))
        .execute(conn)
        .expect("Failed to set the threshold's model");
}

/// An alert raised by a report created `created` seconds after the base timestamp.
///
/// The report starts one second and ends two seconds after it was created, so a window over
/// the wrong one of the three counts other alerts.
fn insert_alert(
    conn: &mut DbConnection,
    base: &SeededBase,
    (threshold, model): (&SeededThreshold, ModelUuid),
    (head, testbed, measure): (i32, i32, i32),
    created: i64,
    status: AlertStatus,
) {
    diesel::insert_into(schema::report::table)
        .values((
            schema::report::uuid.eq(ReportUuid::new()),
            schema::report::project_id.eq(base.project),
            schema::report::head_id.eq(head),
            schema::report::version_id.eq(base.version),
            schema::report::testbed_id.eq(testbed),
            schema::report::adapter.eq(0),
            schema::report::start_time.eq(seconds(created + 1)),
            schema::report::end_time.eq(seconds(created + 2)),
            schema::report::created.eq(seconds(created)),
        ))
        .execute(&mut *conn)
        .expect("Failed to insert a report");
    let report = last_id(conn);
    diesel::insert_into(schema::report_benchmark::table)
        .values((
            schema::report_benchmark::uuid.eq(ReportBenchmarkUuid::new()),
            schema::report_benchmark::report_id.eq(report),
            schema::report_benchmark::iteration.eq(0),
            schema::report_benchmark::benchmark_id.eq(base.benchmark),
            schema::report_benchmark::variant_id.eq(base.variant),
        ))
        .execute(&mut *conn)
        .expect("Failed to insert a report benchmark");
    let report_benchmark = last_id(conn);
    diesel::insert_into(schema::metric::table)
        .values((
            schema::metric::uuid.eq(MetricUuid::new()),
            schema::metric::report_benchmark_id.eq(report_benchmark),
            schema::metric::measure_id.eq(measure),
            schema::metric::name.eq(MetricName::value()),
            schema::metric::value.eq(1000.0),
        ))
        .execute(&mut *conn)
        .expect("Failed to insert a metric");
    let metric = last_id(conn);
    let model_id: i32 = schema::model::table
        .filter(schema::model::uuid.eq(model))
        .select(schema::model::id)
        .first(&mut *conn)
        .expect("Failed to get the model");
    diesel::insert_into(schema::boundary::table)
        .values((
            schema::boundary::uuid.eq(BoundaryUuid::new()),
            schema::boundary::metric_id.eq(metric),
            schema::boundary::threshold_id.eq(threshold.id),
            schema::boundary::model_id.eq(model_id),
            schema::boundary::baseline.eq(Some(1.0)),
            schema::boundary::upper_limit.eq(Some(100.0)),
        ))
        .execute(&mut *conn)
        .expect("Failed to insert a boundary");
    let boundary = last_id(conn);
    diesel::insert_into(schema::alert::table)
        .values((
            schema::alert::uuid.eq(AlertUuid::new()),
            schema::alert::project_id.eq(base.project),
            schema::alert::threshold_id.eq(threshold.id),
            schema::alert::boundary_id.eq(boundary),
            schema::alert::boundary_limit.eq(BoundaryLimit::Upper),
            schema::alert::status.eq(status),
            schema::alert::modified.eq(seconds(created)),
        ))
        .execute(conn)
        .expect("Failed to insert an alert");
}

fn last_id(conn: &mut DbConnection) -> i32 {
    diesel::select(last_insert_rowid())
        .get_result(conn)
        .expect("Failed to get the last inserted ID")
}

fn seconds(offset: i64) -> DateTime {
    DateTime::try_from(base_timestamp().timestamp() + offset).expect("Invalid timestamp")
}

fn millis(offset: i64) -> i64 {
    (base_timestamp().timestamp() + offset) * 1000
}

fn window(start: Option<i64>, end: Option<i64>) -> String {
    let mut query = Vec::new();
    if let Some(start) = start {
        query.push(format!("start_time={}", millis(start)));
    }
    if let Some(end) = end {
        query.push(format!("end_time={}", millis(end)));
    }
    format!("?{}", query.join("&"))
}

/// Each threshold's UUID with its raised and active counts, in list order.
fn counts(body: &Value) -> Vec<(String, u64, u64)> {
    body["thresholds"]
        .as_array()
        .expect("No thresholds")
        .iter()
        .map(|threshold| {
            (
                threshold["uuid"].as_str().expect("No UUID").to_owned(),
                threshold["raised"].as_u64().expect("No raised count"),
                threshold["active"].as_u64().expect("No active count"),
            )
        })
        .collect()
}

/// Every dimension in the tables, by kind, with its name and when it was archived.
fn archived(body: &Value) -> Vec<(&'static str, String, Option<i64>)> {
    let mut dimensions = Vec::new();
    for (kind, table) in [
        ("branch", "branches"),
        ("testbed", "testbeds"),
        ("measure", "measures"),
    ] {
        for dimension in body[table].as_array().expect("No table") {
            dimensions.push((
                kind,
                dimension["name"].as_str().expect("No name").to_owned(),
                dimension["archived"].as_i64(),
            ));
        }
    }
    dimensions
}

async fn get(server: &TestServer, token: &str, slug: &str, query: &str) -> (StatusCode, Value) {
    request(
        server,
        Some(token),
        &format!("/v0/projects/{slug}/console/thresholds{query}"),
    )
    .await
}

async fn get_one(
    server: &TestServer,
    token: &str,
    slug: &str,
    threshold: ThresholdUuid,
) -> (StatusCode, Value) {
    request(
        server,
        Some(token),
        &format!("/v0/projects/{slug}/console/thresholds/{threshold}"),
    )
    .await
}

async fn request(server: &TestServer, token: Option<&str>, path: &str) -> (StatusCode, Value) {
    let mut request = server.client.get(server.api_url(path));
    if let Some(token) = token {
        request = request.header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(token),
        );
    }
    let resp = request.send().await.expect("Request failed");
    let status = resp.status();
    let body = resp.json().await.unwrap_or(Value::Null);
    (status, body)
}

async fn create_key(server: &TestServer, user: &TestUser, slug: &str) -> String {
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{slug}/keys")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&json!({ "name": "console-key" }))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED, "create a project key");
    let key: JsonProjectKeyCreated = resp.json().await.expect("Failed to parse response");
    key.key.as_ref().to_owned()
}
