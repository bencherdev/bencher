#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! Integration tests for what a plot keeps of its view: the metrics it draws, the lines
//! it hides, the line it focuses, and its layout.

use bencher_api_tests::{
    TestServer, TestUser,
    helpers::{base_timestamp, create_empty_variant, get_project_id},
};
use bencher_json::{BenchmarkUuid, BranchUuid, JsonPlot, MeasureUuid, PlotUuid, TestbedUuid};
use bencher_schema::{MIGRATIONS, context::DbConnection, schema};
use diesel::{
    ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _, connection::SimpleConnection as _,
};
use diesel_migrations::MigrationHarness as _;
use http::{Method, StatusCode};
use serde_json::{Value, json};

const PLOT_VIEW_MIGRATION: &str = "20261008120000";

/// The fields a plot made before it could keep a view reads with.
const CLASSIC_FIELDS: [&str; 16] = [
    "benchmarks",
    "branches",
    "created",
    "lower_boundary",
    "lower_value",
    "measures",
    "modified",
    "project",
    "testbeds",
    "title",
    "upper_boundary",
    "upper_value",
    "uuid",
    "window",
    "x_axis",
    "y_axis",
];

struct Dimensions {
    branch: BranchUuid,
    testbed: TestbedUuid,
    benchmark: BenchmarkUuid,
    measure: MeasureUuid,
}

struct Fixture {
    server: TestServer,
    user: TestUser,
    project_slug: String,
    dims: Dimensions,
}

async fn fixture(label: &str) -> Fixture {
    let server = TestServer::new().await;
    let user = server
        .signup("Plot User", &format!("{label}@example.com"))
        .await;
    let org = server.create_org(&user, &format!("Org {label}")).await;
    let project = server
        .create_project(&user, &org, &format!("Project {label}"))
        .await;
    let project_slug = project.slug.to_string();
    let dims = seed_dimensions(&server, get_project_id(&server, &project_slug));
    Fixture {
        server,
        user,
        project_slug,
        dims,
    }
}

#[expect(clippy::expect_used, reason = "test helper seeding plot dimensions")]
fn seed_dimensions(server: &TestServer, project_id: i32) -> Dimensions {
    let now = base_timestamp();
    let branch = BranchUuid::new();
    let testbed = TestbedUuid::new();
    let benchmark = BenchmarkUuid::new();
    let measure = MeasureUuid::new();

    let mut conn = db_conn(server);
    diesel::insert_into(schema::branch::table)
        .values((
            schema::branch::uuid.eq(&branch),
            schema::branch::project_id.eq(project_id),
            schema::branch::name.eq("main"),
            schema::branch::slug.eq(&format!("main-{branch}")),
            schema::branch::created.eq(&now),
            schema::branch::modified.eq(&now),
        ))
        .execute(&mut conn)
        .expect("Failed to insert branch");
    diesel::insert_into(schema::testbed::table)
        .values((
            schema::testbed::uuid.eq(&testbed),
            schema::testbed::project_id.eq(project_id),
            schema::testbed::name.eq("localhost"),
            schema::testbed::slug.eq(&format!("localhost-{testbed}")),
            schema::testbed::created.eq(&now),
            schema::testbed::modified.eq(&now),
        ))
        .execute(&mut conn)
        .expect("Failed to insert testbed");
    diesel::insert_into(schema::benchmark::table)
        .values((
            schema::benchmark::uuid.eq(&benchmark),
            schema::benchmark::project_id.eq(project_id),
            schema::benchmark::name.eq("bench"),
            schema::benchmark::slug.eq(&format!("bench-{benchmark}")),
            schema::benchmark::created.eq(&now),
            schema::benchmark::modified.eq(&now),
        ))
        .execute(&mut conn)
        .expect("Failed to insert benchmark");
    let benchmark_id: i32 = schema::benchmark::table
        .filter(schema::benchmark::uuid.eq(&benchmark))
        .select(schema::benchmark::id)
        .first(&mut conn)
        .expect("Failed to get benchmark ID");
    create_empty_variant(&mut conn, benchmark_id);
    diesel::insert_into(schema::measure::table)
        .values((
            schema::measure::uuid.eq(&measure),
            schema::measure::project_id.eq(project_id),
            schema::measure::name.eq("latency"),
            schema::measure::slug.eq(&format!("latency-{measure}")),
            schema::measure::units.eq("nanoseconds"),
            schema::measure::created.eq(&now),
            schema::measure::modified.eq(&now),
        ))
        .execute(&mut conn)
        .expect("Failed to insert measure");

    Dimensions {
        branch,
        testbed,
        benchmark,
        measure,
    }
}

/// A connection that waits out the server's own, as the server's connections do.
#[expect(clippy::expect_used, reason = "test helper opening a connection")]
fn db_conn(server: &TestServer) -> DbConnection {
    let mut conn = server.db_conn();
    conn.batch_execute("PRAGMA busy_timeout = 5000")
        .expect("Failed to set the busy timeout");
    conn
}

impl Fixture {
    /// A new plot body with every required field, plus `extra`.
    fn new_plot(&self, extra: &Value) -> Value {
        let mut plot = json!({
            "title": "Pinned",
            "lower_value": false,
            "upper_value": false,
            "lower_boundary": false,
            "upper_boundary": true,
            "x_axis": "date_time",
            "window": 2_592_000,
            "branches": [self.dims.branch],
            "testbeds": [self.dims.testbed],
            "benchmarks": [self.dims.benchmark],
            "measures": [self.dims.measure],
        });
        if let (Some(plot), Some(extra)) = (plot.as_object_mut(), extra.as_object()) {
            plot.extend(extra.clone());
        }
        plot
    }

    fn plots_url(&self) -> String {
        self.server
            .api_url(&format!("/v0/projects/{}/plots", self.project_slug))
    }

    fn plot_url(&self, plot: PlotUuid) -> String {
        self.server
            .api_url(&format!("/v0/projects/{}/plots/{plot}", self.project_slug))
    }

    #[expect(clippy::expect_used, reason = "test helper sending a request")]
    async fn send(
        &self,
        method: Method,
        url: String,
        body: Option<&Value>,
    ) -> (StatusCode, String) {
        let mut request = self.server.client.request(method, url).header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&self.user.token),
        );
        if let Some(body) = body {
            request = request.json(body);
        }
        let resp = request.send().await.expect("Request failed");
        let status = resp.status();
        (
            status,
            resp.text().await.expect("Failed to read the response"),
        )
    }

    async fn try_create(&self, extra: &Value) -> (StatusCode, String) {
        self.send(Method::POST, self.plots_url(), Some(&self.new_plot(extra)))
            .await
    }

    async fn try_patch(&self, plot: PlotUuid, patch: &Value) -> (StatusCode, String) {
        self.send(Method::PATCH, self.plot_url(plot), Some(patch))
            .await
    }

    /// Create a plot and return its raw response body.
    async fn create(&self, extra: &Value) -> Value {
        let (status, body) = self.try_create(extra).await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        parse(&body)
    }

    /// Patch a plot and return its raw response body.
    async fn patch(&self, plot: PlotUuid, patch: &Value) -> Value {
        let (status, body) = self.try_patch(plot, patch).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        parse(&body)
    }

    /// Read a plot back, alone and from the list, and return both raw bodies.
    #[expect(clippy::expect_used, reason = "test helper reading a plot")]
    async fn read(&self, plot: PlotUuid) -> (Value, Value) {
        let (status, one) = self.send(Method::GET, self.plot_url(plot), None).await;
        assert_eq!(status, StatusCode::OK, "{one}");
        let (status, list) = self.send(Method::GET, self.plots_url(), None).await;
        assert_eq!(status, StatusCode::OK, "{list}");
        let listed = parse(&list)
            .as_array()
            .and_then(|plots| {
                plots
                    .iter()
                    .find(|listed| listed.get("uuid") == Some(&json!(plot)))
                    .cloned()
            })
            .expect("The list holds the plot");
        (parse(&one), listed)
    }

    /// Assert that every read of a plot carries exactly `view`, and nothing else of it.
    async fn assert_view(&self, plot: &Value, view: &Value) {
        let uuid = uuid_of(plot);
        let (one, listed) = self.read(uuid).await;
        for (read, body) in [("response", plot), ("get", &one), ("list", &listed)] {
            assert_eq!(view_of(body), *view, "{read}: {body}");
        }
    }
}

#[expect(clippy::expect_used, reason = "test helper parsing a response")]
fn parse(body: &str) -> Value {
    serde_json::from_str(body).expect("Failed to parse the response")
}

#[expect(clippy::expect_used, reason = "test helper reading a plot")]
fn uuid_of(plot: &Value) -> PlotUuid {
    serde_json::from_value::<JsonPlot>(plot.clone())
        .expect("Failed to parse the plot")
        .uuid
}

/// The view fields a plot body carries, absent ones left out.
fn view_of(plot: &Value) -> Value {
    let mut view = serde_json::Map::new();
    for field in ["metrics", "hidden", "focus", "layout"] {
        if let Some(value) = plot.get(field) {
            view.insert(field.to_owned(), value.clone());
        }
    }
    Value::Object(view)
}

fn fields_of(plot: &Value) -> Vec<String> {
    let mut fields = plot
        .as_object()
        .map(|plot| plot.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    fields.sort();
    fields
}

// Create, get, and list carry every view field exactly as it was written, with
// duplicate metrics and hidden lines collapsed in the order they were first written.
#[tokio::test]
async fn plot_view_round_trips() {
    let f = fixture("plotviewroundtrip").await;
    let plot = f
        .create(&json!({
            "metrics": ["p99", "value", "p99"],
            "hidden": ["1a2b3c4d5e6", "zz_TOP-9", "1a2b3c4d5e6"],
            "focus": "0123456789abcdef",
            "layout": "stacked",
        }))
        .await;
    f.assert_view(
        &plot,
        &json!({
            "metrics": ["p99", "value"],
            "hidden": ["1a2b3c4d5e6", "zz_TOP-9"],
            "focus": "0123456789abcdef",
            "layout": "stacked",
        }),
    )
    .await;

    let dual = f.create(&json!({ "layout": "dual" })).await;
    f.assert_view(&dual, &json!({ "layout": "dual" })).await;
}

// A patch sets, changes, and clears each view field on its own, and a patch that
// leaves a field out leaves it alone.
#[tokio::test]
async fn plot_view_patch_sets_changes_and_clears() {
    let f = fixture("plotviewpatch").await;
    let plot = f.create(&json!({})).await;
    let uuid = uuid_of(&plot);
    f.assert_view(&plot, &json!({})).await;

    let set = json!({
        "metrics": ["value"],
        "hidden": ["a1"],
        "focus": "a1",
        "layout": "stacked",
    });
    let updated = f.patch(uuid, &set).await;
    f.assert_view(&updated, &set).await;

    let changed = json!({
        "metrics": ["lower_value", "upper_value"],
        "hidden": ["b2", "c3"],
        "focus": "c3",
        "layout": "dual",
    });
    let updated = f.patch(uuid, &changed).await;
    f.assert_view(&updated, &changed).await;

    let untouched = f.patch(uuid, &json!({ "lower_value": true })).await;
    assert_eq!(
        untouched.get("lower_value"),
        Some(&json!(true)),
        "{untouched}"
    );
    f.assert_view(&untouched, &changed).await;

    // A `null` title takes the other variant of the patch, which carries the view too.
    let mut remaining = json!({
        "metrics": ["value", "upper_value"],
        "hidden": ["d4", "c3"],
        "focus": "b2",
        "layout": "stacked",
    });
    let mut retitle = remaining.clone();
    if let Some(retitle) = retitle.as_object_mut() {
        retitle.insert("title".to_owned(), Value::Null);
    }
    let retitled = f.patch(uuid, &retitle).await;
    assert_eq!(retitled.get("title"), Some(&Value::Null), "{retitled}");
    f.assert_view(&retitled, &remaining).await;
    // The classic console clears a title without naming the view, which keeps it.
    let untitled = f
        .patch(uuid, &json!({ "title": null, "lower_value": false }))
        .await;
    f.assert_view(&untitled, &remaining).await;

    for field in ["focus", "metrics", "layout", "hidden"] {
        let cleared = f.patch(uuid, &json!({ field: null })).await;
        if let Some(remaining) = remaining.as_object_mut() {
            remaining.remove(field);
        }
        f.assert_view(&cleared, &remaining).await;
    }
    assert_eq!(remaining, json!({}), "every field was cleared");

    // An empty list clears the lists the way `null` does.
    f.patch(uuid, &json!({ "metrics": ["value"], "hidden": ["a1"] }))
        .await;
    let emptied = f.patch(uuid, &json!({ "metrics": [], "hidden": [] })).await;
    f.assert_view(&emptied, &json!({})).await;
}

// A plot that keeps no view reads exactly as a plot made before plots could keep one,
// however the absence was spelled.
#[tokio::test]
async fn plot_without_a_view_reads_as_before() {
    let f = fixture("plotviewclassic").await;
    for extra in [
        json!({}),
        json!({ "metrics": null, "hidden": null, "focus": null, "layout": null }),
        json!({ "metrics": [], "hidden": [] }),
    ] {
        let plot = f.create(&extra).await;
        let (one, listed) = f.read(uuid_of(&plot)).await;
        for (read, body) in [("response", &plot), ("get", &one), ("list", &listed)] {
            assert_eq!(fields_of(body), CLASSIC_FIELDS, "{extra} {read}: {body}");
        }
    }
}

// The view is refused with a 400 past its caps or with a malformed value, on create
// and on patch, and accepted at its caps.
#[tokio::test]
async fn plot_view_limits_are_refused() {
    let f = fixture("plotviewlimits").await;
    let plot = uuid_of(&f.create(&json!({})).await);

    let metrics = |n: usize| (0..n).map(|i| format!("m{i}")).collect::<Vec<_>>();
    let keys = |n: usize| (0..n).map(|i| format!("k{i}")).collect::<Vec<_>>();
    let refused = [
        (json!({ "metrics": metrics(9) }), "at most 8 metrics"),
        (json!({ "metrics": vec!["value"; 9] }), "at most 8 metrics"),
        (json!({ "metrics": [""] }), "metric name"),
        (json!({ "hidden": keys(65) }), "at most 64 lines"),
        (json!({ "hidden": vec!["k"; 65] }), "at most 64 lines"),
        (json!({ "hidden": ["0123456789abcdefg"] }), "line key"),
        (json!({ "hidden": [""] }), "line key"),
        (json!({ "hidden": ["a/b"] }), "line key"),
        (json!({ "hidden": ["a.b"] }), "line key"),
        (json!({ "focus": "0123456789abcdefg" }), "line key"),
        (json!({ "focus": "" }), "line key"),
        (json!({ "focus": "a+b" }), "line key"),
        (json!({ "layout": "grid" }), "grid"),
    ];
    for (view, refusal) in &refused {
        for (status, body) in [f.try_create(view).await, f.try_patch(plot, view).await] {
            assert_eq!(status, StatusCode::BAD_REQUEST, "{view}: {body}");
            assert!(body.contains(refusal), "{view}: {body}");
        }
    }

    let at_caps = json!({
        "metrics": metrics(8),
        "hidden": keys(64),
        "focus": "0123456789abcdef",
    });
    let created = f.create(&at_caps).await;
    f.assert_view(&created, &at_caps).await;
    let patched = f.patch(plot, &at_caps).await;
    f.assert_view(&patched, &at_caps).await;
}

/// One row per plot: every column a plot had before it could keep a view.
const PLOT_ROWS: &str = "SELECT json_array(id, uuid, project_id, rank, title, lower_value, upper_value, lower_boundary, upper_boundary, x_axis, y_axis, window, hex(parameters), created, modified) AS row FROM plot ORDER BY id";
/// One row per plot: its view columns.
const VIEW_ROWS: &str =
    "SELECT json_array(id, layout, metrics, hidden, focus) AS row FROM plot ORDER BY id";
/// One row per plot component.
const COMPONENT_ROWS: &str =
    "SELECT json_array('branch', plot_id, branch_id, rank) AS row FROM plot_branch
    UNION ALL SELECT json_array('testbed', plot_id, testbed_id, rank) FROM plot_testbed
    UNION ALL SELECT json_array('benchmark', plot_id, benchmark_id, rank) FROM plot_benchmark
    UNION ALL SELECT json_array('measure', plot_id, measure_id, rank) FROM plot_measure
    ORDER BY row";
/// The table `plot` as written.
const PLOT_TABLE: &str =
    "SELECT sql AS row FROM sqlite_master WHERE type = 'table' AND name = 'plot'";
/// Every column `plot` had before it could keep a view, and every foreign key of `plot`.
const PLOT_SCHEMA: &str = "SELECT json_array(name, type, \"notnull\", dflt_value, pk) AS row FROM pragma_table_info('plot') WHERE name NOT IN ('layout', 'metrics', 'hidden', 'focus')
    UNION ALL SELECT json_array(\"table\", \"from\", \"to\", on_update, on_delete) FROM pragma_foreign_key_list('plot')
    ORDER BY row";
/// Every row whose foreign key points nowhere.
const BROKEN_KEYS: &str =
    "SELECT json_array(\"table\", rowid, parent, fkid) AS row FROM pragma_foreign_key_check";
/// Every index on `plot`, as written.
const PLOT_INDEXES: &str = "SELECT json_array(name, sql) AS row FROM sqlite_master WHERE tbl_name = 'plot' AND type = 'index' ORDER BY name";

#[derive(diesel::QueryableByName)]
struct RawRow {
    #[diesel(sql_type = diesel::sql_types::Text)]
    row: String,
}

#[expect(clippy::expect_used, reason = "test helper reading raw rows")]
fn raw_rows(conn: &mut DbConnection, query: &str) -> Vec<String> {
    diesel::sql_query(query)
        .load::<RawRow>(conn)
        .expect("Failed to read the rows")
        .into_iter()
        .map(|raw| raw.row)
        .collect()
}

/// Revert every migration down to and including the plot view migration.
#[expect(clippy::expect_used, reason = "test helper reverting a migration")]
fn revert_migration(conn: &mut DbConnection) {
    conn.batch_execute("PRAGMA foreign_keys = OFF")
        .expect("Failed to disable foreign keys");
    loop {
        let version = conn
            .revert_last_migration(MIGRATIONS)
            .expect("Failed to revert a migration");
        if version.to_string() == PLOT_VIEW_MIGRATION {
            break;
        }
    }
    conn.batch_execute("PRAGMA foreign_keys = ON")
        .expect("Failed to enable foreign keys");
}

#[expect(clippy::expect_used, reason = "test helper applying migrations")]
fn apply_migration(conn: &mut DbConnection) {
    conn.batch_execute("PRAGMA foreign_keys = OFF")
        .expect("Failed to disable foreign keys");
    conn.run_pending_migrations(MIGRATIONS)
        .expect("Failed to apply the migrations");
    conn.batch_execute("PRAGMA foreign_keys = ON")
        .expect("Failed to enable foreign keys");
}

// The migration down and up keeps every plot, column for column, with its components,
// its table, and its index; only the view it kept is dropped on the way down.
// The server keeps serving on its own threads while this test's connection holds a lock.
#[tokio::test(flavor = "multi_thread")]
async fn plot_view_migration_down_and_up_keeps_every_plot() {
    let f = fixture("plotviewmigration").await;
    // A deleted first plot leaves a gap in the IDs, so a renumbered row shows.
    let deleted = uuid_of(&f.create(&json!({})).await);
    let classic = f.create(&json!({})).await;
    let (status, body) = f.send(Method::DELETE, f.plot_url(deleted), None).await;
    assert!(status.is_success(), "{body}");
    let filtered = f
        .create(&json!({ "parameters": [{ "size": 1 }, { "size": 2 }] }))
        .await;
    let viewed = f
        .create(&json!({
            "metrics": ["value"],
            "hidden": ["a1", "b2"],
            "focus": "a1",
            "layout": "stacked",
        }))
        .await;
    let mut before = Vec::new();
    for plot in [&classic, &filtered] {
        before.push(f.read(uuid_of(plot)).await);
    }

    let mut conn = db_conn(&f.server);
    let rows = raw_rows(&mut conn, PLOT_ROWS);
    let components = raw_rows(&mut conn, COMPONENT_ROWS);
    let indexes = raw_rows(&mut conn, PLOT_INDEXES);
    let table = raw_rows(&mut conn, PLOT_TABLE);
    let schema = raw_rows(&mut conn, PLOT_SCHEMA);
    assert_eq!(rows.len(), 3, "{rows:?}");
    assert_eq!(components.len(), 12, "{components:?}");

    revert_migration(&mut conn);
    assert_eq!(
        raw_rows(&mut conn, PLOT_ROWS),
        rows,
        "down keeps every plot"
    );
    assert_eq!(
        raw_rows(&mut conn, COMPONENT_ROWS),
        components,
        "down keeps every component"
    );
    assert_eq!(
        raw_rows(&mut conn, PLOT_INDEXES),
        indexes,
        "down keeps every index"
    );
    assert_eq!(
        raw_rows(&mut conn, PLOT_SCHEMA),
        schema,
        "down keeps every constraint"
    );
    assert_eq!(
        raw_rows(&mut conn, BROKEN_KEYS),
        Vec::<String>::new(),
        "down breaks no key"
    );

    apply_migration(&mut conn);
    assert_eq!(raw_rows(&mut conn, PLOT_ROWS), rows, "up keeps every plot");
    assert_eq!(
        raw_rows(&mut conn, COMPONENT_ROWS),
        components,
        "up keeps every component"
    );
    assert_eq!(
        raw_rows(&mut conn, PLOT_INDEXES),
        indexes,
        "up keeps every index"
    );
    assert_eq!(
        raw_rows(&mut conn, PLOT_TABLE),
        table,
        "up writes the same table"
    );
    assert_eq!(
        raw_rows(&mut conn, BROKEN_KEYS),
        Vec::<String>::new(),
        "up breaks no key"
    );
    let ids = raw_rows(
        &mut conn,
        "SELECT CAST(id AS TEXT) AS row FROM plot ORDER BY id",
    );
    let unviewed = ids
        .iter()
        .map(|id| format!("[{id},null,null,null,null]"))
        .collect::<Vec<_>>();
    assert_eq!(raw_rows(&mut conn, VIEW_ROWS), unviewed, "the view is gone");

    let mut after = Vec::new();
    for plot in [&classic, &filtered] {
        after.push(f.read(uuid_of(plot)).await);
    }
    assert_eq!(after, before, "a plot without a view reads the same");
    f.assert_view(&f.read(uuid_of(&viewed)).await.0, &json!({}))
        .await;
}
