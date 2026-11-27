#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::tests_outside_test_module,
    clippy::too_many_lines,
    reason = "integration test file"
)]
//! The second model slot of a threshold, `changepoint_model`, beside `model`.

use bencher_api_tests::{TestServer, TestUser};
use bencher_json::{ModelUuid, ThresholdUuid};
use bencher_schema::{
    MIGRATIONS,
    context::DbConnection,
    model::project::threshold::{ThresholdId, model::ModelId},
    schema,
};
use diesel::{
    ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _, connection::SimpleConnection as _,
};
use diesel_migrations::MigrationHarness as _;
use http::{Method, StatusCode};
use serde_json::{Value, json};

const CHANGEPOINT_MODEL_MIGRATION: &str = "20261118120000";
const SIGNIFICANCE_MIGRATION: &str = "20261124120000";

/// The refusal of a metric test in the changepoint slot.
const METRIC_TEST_REFUSAL: &str =
    "checks a metric, so it belongs in `model`, not in `changepoint_model`";
/// The refusal of a line test in the model slot.
const LINE_TEST_REFUSAL: &str =
    "checks a line, so it belongs in `changepoint_model`, not in `model`";

/// A model whose test checks a metric, loose enough to compute a boundary from a short history
/// and tight enough that a hundredfold jump is an outlier.
fn metric_model() -> Value {
    json!({
        "test": "t_test",
        "min_sample_size": 2,
        "max_sample_size": 64,
        "upper_boundary": 0.98,
    })
}

/// A model whose test checks a line, with the count of confirming scans it stores by default.
fn changepoint_model() -> Value {
    json!({
        "test": "changepoint",
        "max_sample_size": 50,
        "upper_boundary": 0.05,
        "significance": 0.001,
        "confirmations": 5,
    })
}

/// A changepoint model that names no count of confirming scans.
fn changepoint_model_without_confirmations() -> Value {
    let mut model = changepoint_model();
    model
        .as_object_mut()
        .expect("a model")
        .remove("confirmations");
    model
}

/// A project with a branch, testbed, and measure, and on them the bare threshold with a model.
struct Fixture {
    user: TestUser,
    slug: String,
    threshold: Value,
}

impl Fixture {
    async fn new(server: &TestServer, label: &str) -> Self {
        let user = server
            .signup("Test User", &format!("cp{label}@example.com"))
            .await;
        let org = server.create_org(&user, &format!("CP Org {label}")).await;
        let project = server
            .create_project(&user, &org, &format!("CP Project {label}"))
            .await;
        let slug = project.slug.to_string();
        for (resource, body) in [
            ("branches", json!({ "name": "cp-branch" })),
            ("testbeds", json!({ "name": "cp-testbed" })),
            (
                "measures",
                json!({ "name": "latency", "units": "nanoseconds" }),
            ),
        ] {
            let path = format!("/v0/projects/{slug}/{resource}");
            let (status, body) = send(server, &user, Method::POST, &path, Some(&body)).await;
            assert_eq!(status, StatusCode::CREATED, "{resource}: {body}");
        }
        let mut fixture = Self {
            user,
            slug,
            threshold: Value::Null,
        };
        let (status, threshold) = fixture.create(server, &new_threshold(None)).await;
        assert_eq!(status, StatusCode::CREATED, "{threshold}");
        fixture.threshold = threshold;
        fixture
    }

    fn thresholds(&self) -> String {
        format!("/v0/projects/{}/thresholds", self.slug)
    }

    fn threshold_path(&self) -> String {
        format!("{}/{}", self.thresholds(), uuid(&self.threshold))
    }

    async fn create(&self, server: &TestServer, body: &Value) -> (StatusCode, Value) {
        send(
            server,
            &self.user,
            Method::POST,
            &self.thresholds(),
            Some(body),
        )
        .await
    }

    async fn get(&self, server: &TestServer, path: &str) -> (StatusCode, Value) {
        send(server, &self.user, Method::GET, path, None).await
    }

    async fn put(&self, server: &TestServer, body: &Value) -> (StatusCode, Value) {
        send(
            server,
            &self.user,
            Method::PUT,
            &self.threshold_path(),
            Some(body),
        )
        .await
    }
}

/// A threshold on the fixture's branch, testbed, and measure, checking `metric` when given.
fn new_threshold(metric: Option<&str>) -> Value {
    let mut threshold = json!({
        "branch": "cp-branch",
        "testbed": "cp-testbed",
        "measure": "latency",
    });
    if let Some(metric) = metric {
        threshold["metric"] = metric.into();
    }
    merge(threshold, &metric_model())
}

fn merge(mut into: Value, from: &Value) -> Value {
    if let (Some(into), Some(from)) = (into.as_object_mut(), from.as_object()) {
        into.extend(from.clone());
    }
    into
}

fn uuid(json: &Value) -> &str {
    json["uuid"].as_str().expect("a UUID")
}

async fn send(
    server: &TestServer,
    user: &TestUser,
    method: Method,
    path: &str,
    body: Option<&Value>,
) -> (StatusCode, Value) {
    let mut request = server.client.request(method, server.api_url(path)).header(
        bencher_json::AUTHORIZATION,
        bencher_json::bearer_header(&user.token),
    );
    if let Some(body) = body {
        request = request.json(body);
    }
    let resp = request.send().await.expect("Request failed");
    let status = resp.status();
    let text = resp.text().await.expect("Failed to read the response");
    let body = if text.is_empty() {
        Value::Null
    } else {
        serde_json::from_str(&text).unwrap_or(Value::String(text))
    };
    (status, body)
}

// Every threshold response names the changepoint slot, and it is empty until a model fills it.
#[tokio::test]
async fn a_threshold_carries_an_empty_changepoint_model() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "empty").await;
    assert_eq!(f.threshold.get("changepoint_model"), Some(&Value::Null));

    let (status, threshold) = f.get(&server, &f.threshold_path()).await;
    assert_eq!(status, StatusCode::OK, "{threshold}");
    assert_eq!(threshold.get("changepoint_model"), Some(&Value::Null));

    let (status, thresholds) = f.get(&server, &f.thresholds()).await;
    assert_eq!(status, StatusCode::OK, "{thresholds}");
    let listed = thresholds.as_array().expect("a list of thresholds");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].get("changepoint_model"), Some(&Value::Null));

    let (status, threshold) = f
        .put(
            &server,
            &json!({ "test": "percentage", "upper_boundary": 0.1 }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{threshold}");
    assert_eq!(threshold.get("changepoint_model"), Some(&Value::Null));
}

// A model whose test checks a metric goes in `model`, so the changepoint slot refuses it on a
// create, an update, and an update that removes the model, and the refusal changes nothing.
#[tokio::test]
async fn a_metric_test_in_the_changepoint_slot_is_refused() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "refused").await;

    let create = merge(
        new_threshold(Some("p99")),
        &json!({ "changepoint_model": metric_model() }),
    );
    let (status, body) = f.create(&server, &create).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.to_string().contains(METRIC_TEST_REFUSAL), "{body}");
    let (_, thresholds) = f.get(&server, &f.thresholds()).await;
    assert_eq!(
        thresholds.as_array().map(Vec::len),
        Some(1),
        "the refused create made no threshold"
    );

    for update in [
        merge(
            json!({ "test": "percentage", "upper_boundary": 0.1 }),
            &json!({ "changepoint_model": metric_model() }),
        ),
        json!({ "test": null, "changepoint_model": metric_model() }),
    ] {
        let (status, body) = f.put(&server, &update).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert!(body.to_string().contains(METRIC_TEST_REFUSAL), "{body}");
        let (_, threshold) = f.get(&server, &f.threshold_path()).await;
        assert_eq!(
            threshold["model"]["uuid"], f.threshold["model"]["uuid"],
            "the refused update kept the model"
        );
    }
}

/// One row per threshold: every column a threshold had before the changepoint slot.
const THRESHOLD_ROWS: &str = "SELECT json_array(id, uuid, project_id, branch_id, testbed_id, hex(parameters), measure_id, metric, model_id, created, modified) AS row FROM threshold ORDER BY id";
/// One row per model.
const MODEL_ROWS: &str = "SELECT json_array(id, uuid, threshold_id, test, min_sample_size, max_sample_size, window, lower_boundary, upper_boundary, created, replaced) AS row FROM model ORDER BY id";
/// The columns of `threshold`, in order.
const THRESHOLD_COLUMNS: &str =
    "SELECT name AS row FROM pragma_table_info('threshold') ORDER BY cid";
/// Every foreign key of `threshold`.
const THRESHOLD_KEYS: &str = "SELECT json_array(\"table\", \"from\", \"to\", on_update, on_delete) AS row FROM pragma_foreign_key_list('threshold') ORDER BY row";
/// Every index on `threshold`, as written.
const THRESHOLD_INDEXES: &str = "SELECT json_array(name, sql) AS row FROM sqlite_master WHERE tbl_name = 'threshold' AND type = 'index' ORDER BY name";
/// Every threshold's changepoint slot.
const CHANGEPOINT_SLOTS: &str =
    "SELECT json_array(id, changepoint_model_id) AS row FROM threshold ORDER BY id";
/// Every row whose foreign key points nowhere.
const BROKEN_KEYS: &str =
    "SELECT json_array(\"table\", rowid, parent, fkid) AS row FROM pragma_foreign_key_check";

// The migration down and up keeps every threshold and its model, column for column, with every
// key and index, and every threshold comes up with an empty changepoint slot right after `model_id`.
// The server keeps serving on its own threads while this test's connection holds a lock.
#[tokio::test(flavor = "multi_thread")]
async fn changepoint_model_migration_down_and_up_keeps_every_threshold() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "migration").await;
    let named = merge(
        new_threshold(Some("p99")),
        &json!({ "parameters": [{ "size": 1 }] }),
    );
    let (status, body) = f.create(&server, &named).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let (status, body) = f.create(&server, &new_threshold(Some("p50"))).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let removed = format!("{}/{}", f.thresholds(), uuid(&body));
    let (status, body) = send(
        &server,
        &f.user,
        Method::PUT,
        &removed,
        Some(&json!({ "test": null })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let mut conn = server.db_conn();
    let thresholds = raw_rows(&mut conn, THRESHOLD_ROWS);
    let models = raw_rows(&mut conn, MODEL_ROWS);
    let columns = raw_rows(&mut conn, THRESHOLD_COLUMNS);
    let keys = raw_rows(&mut conn, THRESHOLD_KEYS);
    let indexes = raw_rows(&mut conn, THRESHOLD_INDEXES);
    assert_eq!(thresholds.len(), 3, "{thresholds:?}");
    assert_eq!(models.len(), 3, "{models:?}");
    let model_column = columns
        .iter()
        .position(|column| column == "model_id")
        .expect("a model column");
    assert_eq!(
        columns.get(model_column + 1).map(String::as_str),
        Some("changepoint_model_id"),
        "the changepoint slot sits right after the model"
    );

    revert_migration(&mut conn, CHANGEPOINT_MODEL_MIGRATION);
    assert_eq!(
        raw_rows(&mut conn, THRESHOLD_ROWS),
        thresholds,
        "down keeps every threshold"
    );
    assert_eq!(
        raw_rows(&mut conn, MODEL_ROWS),
        models,
        "down keeps every model"
    );
    let without_slot = columns
        .iter()
        .filter(|column| *column != "changepoint_model_id")
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        raw_rows(&mut conn, THRESHOLD_COLUMNS),
        without_slot,
        "down drops the slot"
    );
    let down_keys = raw_rows(&mut conn, THRESHOLD_KEYS);
    assert_eq!(
        down_keys.len() + 1,
        keys.len(),
        "down drops the slot's key: {down_keys:?}"
    );
    let down_indexes = raw_rows(&mut conn, THRESHOLD_INDEXES);
    assert_eq!(
        down_indexes.len() + 1,
        indexes.len(),
        "down drops the slot's index: {down_indexes:?}"
    );
    assert!(
        down_indexes.iter().all(|index| indexes.contains(index)),
        "down keeps every other index: {down_indexes:?}"
    );
    assert_eq!(
        raw_rows(&mut conn, BROKEN_KEYS),
        Vec::<String>::new(),
        "down breaks no key"
    );

    apply_migration(&mut conn);
    assert_eq!(
        raw_rows(&mut conn, THRESHOLD_ROWS),
        thresholds,
        "up keeps every threshold"
    );
    assert_eq!(
        raw_rows(&mut conn, MODEL_ROWS),
        models,
        "up keeps every model"
    );
    assert_eq!(
        raw_rows(&mut conn, THRESHOLD_COLUMNS),
        columns,
        "up restores the slot"
    );
    assert_eq!(
        raw_rows(&mut conn, THRESHOLD_KEYS),
        keys,
        "up restores every key"
    );
    assert_eq!(
        raw_rows(&mut conn, THRESHOLD_INDEXES),
        indexes,
        "up restores every index"
    );
    assert!(
        raw_rows(&mut conn, CHANGEPOINT_SLOTS)
            .iter()
            .all(|slot| slot.ends_with(",null]")),
        "every changepoint slot comes up empty"
    );
    assert_eq!(
        raw_rows(&mut conn, BROKEN_KEYS),
        Vec::<String>::new(),
        "up breaks no key"
    );
    drop(conn);

    let (status, threshold) = f.get(&server, &f.threshold_path()).await;
    assert_eq!(status, StatusCode::OK, "{threshold}");
    assert_eq!(threshold["model"]["uuid"], f.threshold["model"]["uuid"]);
}

#[derive(diesel::QueryableByName)]
struct RawRow {
    #[diesel(sql_type = diesel::sql_types::Text)]
    row: String,
}

fn raw_rows(conn: &mut DbConnection, query: &str) -> Vec<String> {
    diesel::sql_query(query)
        .load::<RawRow>(conn)
        .expect("Failed to read the rows")
        .into_iter()
        .map(|raw| raw.row)
        .collect()
}

/// Revert every migration down to and including this one.
fn revert_migration(conn: &mut DbConnection, migration: &str) {
    conn.batch_execute("PRAGMA foreign_keys = OFF")
        .expect("Failed to disable foreign keys");
    loop {
        let version = conn
            .revert_last_migration(MIGRATIONS)
            .expect("Failed to revert a migration");
        if version.to_string() == migration {
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

/// Fill the changepoint slot of one of the fixture's thresholds through the API, keeping its
/// model, and return the changepoint model.
async fn fill_changepoint_model(server: &TestServer, f: &Fixture, threshold: &str) -> ModelUuid {
    fill_changepoint_model_with(server, f, threshold, &changepoint_model()).await
}

async fn fill_changepoint_model_with(
    server: &TestServer,
    f: &Fixture,
    threshold: &str,
    model: &Value,
) -> ModelUuid {
    let path = format!("{}/{threshold}", f.thresholds());
    let update = merge(metric_model(), &json!({ "changepoint_model": model }));
    let (status, body) = send(server, &f.user, Method::PUT, &path, Some(&update)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    model_uuid(&body, "changepoint_model").expect("a changepoint model")
}

fn threshold_id(server: &TestServer, threshold: &str) -> ThresholdId {
    let uuid: ThresholdUuid = threshold.parse().expect("a threshold UUID");
    schema::threshold::table
        .filter(schema::threshold::uuid.eq(uuid))
        .select(schema::threshold::id)
        .first(&mut server.db_conn())
        .expect("Failed to load the threshold id")
}

/// The model each slot of a threshold holds.
fn slots(server: &TestServer, threshold: &str) -> (Option<ModelUuid>, Option<ModelUuid>) {
    let threshold_id = threshold_id(server, threshold);
    let mut conn = server.db_conn();
    let (model_id, changepoint_model_id) = schema::threshold::table
        .filter(schema::threshold::id.eq(threshold_id))
        .select((
            schema::threshold::model_id,
            schema::threshold::changepoint_model_id,
        ))
        .first::<(Option<ModelId>, Option<ModelId>)>(&mut conn)
        .expect("Failed to load the slots");
    let mut uuid = |model_id: Option<ModelId>| {
        model_id.map(|model_id| {
            schema::model::table
                .filter(schema::model::id.eq(model_id))
                .select(schema::model::uuid)
                .first::<ModelUuid>(&mut conn)
                .expect("Failed to load the model")
        })
    };
    (uuid(model_id), uuid(changepoint_model_id))
}

/// Whether a newer model, or the removal of its slot, replaced the model.
fn is_replaced(server: &TestServer, model: ModelUuid) -> bool {
    schema::model::table
        .filter(schema::model::uuid.eq(model))
        .select(schema::model::replaced)
        .first::<Option<i64>>(&mut server.db_conn())
        .expect("Failed to load the model")
        .is_some()
}

/// Every threshold whose slot holds a model that is replaced or gone.
const DANGLING_SLOTS: &str = "SELECT json_array(threshold.id, slot.model_id) AS row FROM threshold
    JOIN (SELECT id AS threshold_id, model_id FROM threshold WHERE model_id IS NOT NULL
        UNION ALL SELECT id, changepoint_model_id FROM threshold WHERE changepoint_model_id IS NOT NULL) AS slot
        ON slot.threshold_id = threshold.id
    LEFT JOIN model ON model.id = slot.model_id
    WHERE model.id IS NULL OR model.replaced IS NOT NULL OR model.threshold_id != threshold.id";

fn model_uuid(json: &Value, slot: &str) -> Option<ModelUuid> {
    json[slot]["uuid"]
        .as_str()
        .map(|uuid| uuid.parse().expect("a model UUID"))
}

// An update that does not name the changepoint slot keeps it, through a replaced model, and one
// that sets it to null empties it and replaces the model it held.
#[tokio::test]
async fn an_update_keeps_or_clears_the_changepoint_slot() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "update").await;
    let threshold = uuid(&f.threshold).to_owned();
    let changepoint = fill_changepoint_model(&server, &f, &threshold).await;
    let first = model_uuid(&f.threshold, "model").expect("a model");

    let (status, body) = f.get(&server, &f.threshold_path()).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(model_uuid(&body, "changepoint_model"), Some(changepoint));

    let (status, body) = f
        .put(
            &server,
            &json!({ "test": "percentage", "upper_boundary": 0.1 }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let second = model_uuid(&body, "model").expect("a model");
    assert_ne!(second, first);
    assert_eq!(model_uuid(&body, "changepoint_model"), Some(changepoint));
    assert!(
        is_replaced(&server, first),
        "the new model replaced the old"
    );
    assert!(
        !is_replaced(&server, changepoint),
        "the changepoint model stays current"
    );
    assert_eq!(
        slots(&server, &threshold),
        (Some(second), Some(changepoint))
    );

    let (status, body) = f
        .put(
            &server,
            &json!({ "test": "percentage", "upper_boundary": 0.1, "changepoint_model": null }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body.get("changepoint_model"), Some(&Value::Null));
    assert_eq!(
        model_uuid(&body, "model"),
        Some(second),
        "the same model is no change"
    );
    assert!(
        is_replaced(&server, changepoint),
        "the removal replaced the changepoint model"
    );
    assert_eq!(slots(&server, &threshold), (Some(second), None));

    let (status, body) = f
        .put(&server, &json!({ "test": null, "changepoint_model": null }))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(slots(&server, &threshold), (None, None));
    assert_eq!(
        raw_rows(&mut server.db_conn(), DANGLING_SLOTS),
        Vec::<String>::new(),
        "no slot holds a replaced model"
    );
}

/// Post a report onto the fixture's branch and testbed, with one result for `latency`.
async fn post_report(
    server: &TestServer,
    f: &Fixture,
    day: usize,
    value: f64,
    bmf_version: u8,
    thresholds: Option<Value>,
) -> Value {
    let (status, body) = send_report(server, f, day, value, bmf_version, thresholds).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body
}

async fn send_report(
    server: &TestServer,
    f: &Fixture,
    day: usize,
    value: f64,
    bmf_version: u8,
    thresholds: Option<Value>,
) -> (StatusCode, Value) {
    let report = report_body(day, value, bmf_version, thresholds.as_ref());
    let path = format!("/v0/projects/{}/reports", f.slug);
    send(server, &f.user, Method::POST, &path, Some(&report)).await
}

fn report_body(day: usize, value: f64, bmf_version: u8, thresholds: Option<&Value>) -> Value {
    let measures = json!({ "latency": { "value": value } });
    let results = if bmf_version == 0 {
        json!({ "bench": measures })
    } else {
        json!({ "bench": [{ "parameters": {}, "measures": measures }] })
    };
    json!({
        "branch": "cp-branch",
        "testbed": "cp-testbed",
        "start_time": format!("2024-01-{day:02}T00:00:00Z"),
        "end_time": format!("2024-01-{day:02}T00:01:00Z"),
        "results": [results.to_string()],
        "bmf_version": bmf_version,
        "thresholds": thresholds,
    })
}

// A report that names a threshold's model keeps its changepoint model, and a reset clears every
// slot the report did not name: the changepoint slot of the threshold it names, and both slots of
// one it does not.
#[tokio::test]
async fn a_reset_clears_an_unnamed_changepoint_slot() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "reset").await;
    let bare = uuid(&f.threshold).to_owned();
    let (status, named) = f.create(&server, &new_threshold(Some("p99"))).await;
    assert_eq!(status, StatusCode::CREATED, "{named}");
    let named = uuid(&named).to_owned();
    let bare_changepoint = fill_changepoint_model(&server, &f, &bare).await;
    let named_changepoint = fill_changepoint_model(&server, &f, &named).await;
    let (bare_model, _) = slots(&server, &bare);
    let (named_model, _) = slots(&server, &named);

    let models = json!({ "latency": metric_model() });
    post_report(&server, &f, 1, 1.0, 0, Some(json!({ "models": models }))).await;
    assert_eq!(slots(&server, &bare), (bare_model, Some(bare_changepoint)));
    assert_eq!(
        slots(&server, &named),
        (named_model, Some(named_changepoint))
    );

    let entries = json!([{ "measure": "latency", "metric": "value", "model": metric_model() }]);
    post_report(
        &server,
        &f,
        2,
        1.0,
        1,
        Some(json!({ "models": entries, "reset": true })),
    )
    .await;
    assert_eq!(slots(&server, &bare), (bare_model, None));
    assert_eq!(slots(&server, &named), (None, None));
    for model in [bare_changepoint, named_changepoint] {
        assert!(is_replaced(&server, model), "the reset replaced {model}");
    }
    assert_eq!(
        raw_rows(&mut server.db_conn(), DANGLING_SLOTS),
        Vec::<String>::new(),
        "no slot holds a replaced model"
    );
}

/// The one threshold on a branch, as the API serves it.
async fn branch_threshold(server: &TestServer, f: &Fixture, branch: &str) -> Value {
    let (status, thresholds) = f
        .get(server, &format!("{}?branch={branch}", f.thresholds()))
        .await;
    assert_eq!(status, StatusCode::OK, "{thresholds}");
    let mut thresholds = thresholds
        .as_array()
        .cloned()
        .expect("a list of thresholds");
    assert_eq!(thresholds.len(), 1, "{thresholds:?}");
    thresholds.remove(0)
}

/// What a model checks with, without the row it is or the fields it does not set.
fn parameters(model: &Value) -> Value {
    let mut model = model.clone();
    if let Some(model) = model.as_object_mut() {
        for row in ["uuid", "created", "replaced"] {
            model.remove(row);
        }
        model.retain(|_, value| !value.is_null());
    }
    model
}

// A start point clone copies a filled changepoint slot as a new model with the same parameters,
// and a reset clone empties it once the start point's slot is empty.
#[tokio::test]
async fn a_clone_copies_the_changepoint_slot() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "clone").await;
    // A start point is a version, so the branch needs a report to be cloned from.
    post_report(&server, &f, 1, 1.0, 0, None).await;
    let source = uuid(&f.threshold).to_owned();
    let changepoint = fill_changepoint_model(&server, &f, &source).await;
    let (_, source_json) = f.get(&server, &f.threshold_path()).await;

    let branches = format!("/v0/projects/{}/branches", f.slug);
    let clone = json!({
        "name": "feature",
        "start_point": { "branch": "cp-branch", "clone_thresholds": true }
    });
    let (status, body) = send(&server, &f.user, Method::POST, &branches, Some(&clone)).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let copy = branch_threshold(&server, &f, "feature").await;
    let copy_changepoint =
        model_uuid(&copy, "changepoint_model").expect("a cloned changepoint model");
    assert_ne!(copy_changepoint, changepoint, "the clone is a new model");
    assert_eq!(
        parameters(&copy["changepoint_model"]),
        parameters(&source_json["changepoint_model"])
    );
    assert_eq!(
        parameters(&copy["model"]),
        parameters(&source_json["model"])
    );
    let copy = uuid(&copy).to_owned();
    assert_eq!(slots(&server, &copy).1, Some(copy_changepoint));

    let (status, body) = f
        .put(
            &server,
            &merge(metric_model(), &json!({ "changepoint_model": null })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let reset = json!({
        "start_point": { "branch": "cp-branch", "clone_thresholds": true, "reset": true }
    });
    let path = format!("{branches}/feature");
    let (status, body) = send(&server, &f.user, Method::PATCH, &path, Some(&reset)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (copy_model, copy_slot) = slots(&server, &copy);
    assert!(copy_model.is_some(), "the copy keeps its model");
    assert_eq!(
        copy_slot, None,
        "the copy follows its start point's empty slot"
    );
    assert!(is_replaced(&server, copy_changepoint));
    assert_eq!(
        raw_rows(&mut server.db_conn(), DANGLING_SLOTS),
        Vec::<String>::new(),
        "no slot holds a replaced model"
    );
}

// A reset clone follows a new changepoint model on its start point: the copy's changepoint slot
// takes a new model with the new parameters, its old one is replaced, and its model stays.
#[tokio::test]
async fn a_reset_clone_replaces_a_filled_changepoint_slot() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "resync").await;
    // A start point is a version, so the branch needs a report to be cloned from.
    post_report(&server, &f, 1, 1.0, 0, None).await;
    let source = uuid(&f.threshold).to_owned();
    fill_changepoint_model(&server, &f, &source).await;

    let branches = format!("/v0/projects/{}/branches", f.slug);
    let clone = json!({
        "name": "feature",
        "start_point": { "branch": "cp-branch", "clone_thresholds": true }
    });
    let (status, body) = send(&server, &f.user, Method::POST, &branches, Some(&clone)).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let copy = uuid(&branch_threshold(&server, &f, "feature").await).to_owned();
    let (copy_model, copy_changepoint) = slots(&server, &copy);
    let copy_changepoint = copy_changepoint.expect("a cloned changepoint model");

    let source_changepoint = fill_changepoint_model_with(
        &server,
        &f,
        &source,
        &merge(
            changepoint_model(),
            &json!({ "max_sample_size": 25, "upper_boundary": 0.2 }),
        ),
    )
    .await;
    let (_, source_json) = f.get(&server, &f.threshold_path()).await;
    let reset = json!({
        "start_point": { "branch": "cp-branch", "clone_thresholds": true, "reset": true }
    });
    let path = format!("{branches}/feature");
    let (status, body) = send(&server, &f.user, Method::PATCH, &path, Some(&reset)).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (model, changepoint) = slots(&server, &copy);
    assert_eq!(model, copy_model, "the copy keeps its model");
    let changepoint = changepoint.expect("a new changepoint model");
    assert_ne!(changepoint, copy_changepoint, "the copy took a new model");
    assert_ne!(changepoint, source_changepoint, "the copy is a new model");
    assert!(!is_replaced(&server, changepoint));
    assert!(
        is_replaced(&server, copy_changepoint),
        "the new model replaced the old"
    );
    let copy_json = branch_threshold(&server, &f, "feature").await;
    assert_eq!(
        parameters(&copy_json["changepoint_model"]),
        parameters(&source_json["changepoint_model"])
    );
    assert_eq!(
        raw_rows(&mut server.db_conn(), DANGLING_SLOTS),
        Vec::<String>::new(),
        "no slot holds a replaced model"
    );
}

// Deleting a threshold deletes the models of both its slots and breaks no key.
#[tokio::test]
async fn a_threshold_delete_leaves_no_changepoint_model() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "delete").await;
    let threshold = uuid(&f.threshold).to_owned();
    let threshold_id = threshold_id(&server, &threshold);
    fill_changepoint_model(&server, &f, &threshold).await;

    let (status, body) = send(&server, &f.user, Method::DELETE, &f.threshold_path(), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let models = schema::model::table
        .filter(schema::model::threshold_id.eq(threshold_id))
        .count()
        .get_result::<i64>(&mut server.db_conn())
        .expect("Failed to count the models");
    assert_eq!(models, 0, "the delete took the models of both slots");
    let mut conn = server.db_conn();
    assert_eq!(
        raw_rows(&mut conn, BROKEN_KEYS),
        Vec::<String>::new(),
        "the delete breaks no key"
    );
    assert_eq!(raw_rows(&mut conn, DANGLING_SLOTS), Vec::<String>::new());
}

// A branch, testbed, or measure that a threshold hangs off is not deleted, so its slots stay, and a
// hard project delete takes the threshold with the models of both its slots and breaks no key.
#[tokio::test]
async fn a_dimension_or_project_delete_leaves_no_changepoint_model() {
    let server = TestServer::new().await;
    // The first user to sign up is the server admin, who may hard delete a project.
    let f = Fixture::new(&server, "dimensions").await;
    let threshold = uuid(&f.threshold).to_owned();
    let threshold_id = threshold_id(&server, &threshold);
    let changepoint = fill_changepoint_model(&server, &f, &threshold).await;

    for dimension in [
        "branches/cp-branch",
        "testbeds/cp-testbed",
        "measures/latency",
    ] {
        let path = format!("/v0/projects/{}/{dimension}", f.slug);
        let (status, body) = send(&server, &f.user, Method::DELETE, &path, None).await;
        assert_eq!(status, StatusCode::CONFLICT, "{dimension}: {body}");
        assert_eq!(
            slots(&server, &threshold).1,
            Some(changepoint),
            "{dimension}"
        );
    }

    let path = format!("/v0/projects/{}?hard=true", f.slug);
    let (status, body) = send(&server, &f.user, Method::DELETE, &path, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let models = schema::model::table
        .filter(schema::model::threshold_id.eq(threshold_id))
        .count()
        .get_result::<i64>(&mut server.db_conn())
        .expect("Failed to count the models");
    assert_eq!(models, 0, "the delete took the models of both slots");
    assert_eq!(
        raw_rows(&mut server.db_conn(), BROKEN_KEYS),
        Vec::<String>::new(),
        "the delete breaks no key"
    );
}

// A view pinned to a model shows it in its slot and the other slot's current model, a pin refuses
// a model that belongs in the other slot, and a pin to another threshold's model is not found.
#[tokio::test]
async fn a_pinned_view_shows_each_slot() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "pinned").await;
    let threshold = uuid(&f.threshold).to_owned();
    let first_changepoint = fill_changepoint_model(&server, &f, &threshold).await;
    let changepoint = fill_changepoint_model_with(
        &server,
        &f,
        &threshold,
        &merge(changepoint_model(), &json!({ "max_sample_size": 25 })),
    )
    .await;
    let first = model_uuid(&f.threshold, "model").expect("a model");
    let (status, body) = f
        .put(
            &server,
            &json!({ "test": "percentage", "upper_boundary": 0.1 }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let second = model_uuid(&body, "model").expect("a model");
    let (status, other) = f.create(&server, &new_threshold(Some("p99"))).await;
    assert_eq!(status, StatusCode::CREATED, "{other}");
    let other = model_uuid(&other, "model").expect("a model");

    let path = f.threshold_path();
    let (status, body) = f.get(&server, &format!("{path}?model={first}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(model_uuid(&body, "model"), Some(first));
    assert_eq!(model_uuid(&body, "changepoint_model"), Some(changepoint));

    let (status, body) = f
        .get(
            &server,
            &format!("{path}?changepoint_model={first_changepoint}"),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(model_uuid(&body, "model"), Some(second));
    assert_eq!(
        model_uuid(&body, "changepoint_model"),
        Some(first_changepoint)
    );

    for (query, refusal) in [
        (format!("changepoint_model={first}"), METRIC_TEST_REFUSAL),
        (format!("model={changepoint}"), LINE_TEST_REFUSAL),
    ] {
        let (status, body) = f.get(&server, &format!("{path}?{query}")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{query}: {body}");
        assert!(body.to_string().contains(refusal), "{query}: {body}");
    }
    for query in [
        format!("model={other}"),
        format!("changepoint_model={other}"),
    ] {
        let (status, body) = f.get(&server, &format!("{path}?{query}")).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{query}: {body}");
    }
}

// An alert view shows the model that raised the alert and the threshold's current changepoint
// model.
#[tokio::test]
async fn an_alert_view_shows_the_current_changepoint_model() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "alert").await;
    for (day, value) in [10.0, 10.1, 9.9, 10.0, 10.2, 1000.0]
        .into_iter()
        .enumerate()
    {
        post_report(&server, &f, day + 1, value, 0, None).await;
    }
    let alerts = format!("/v0/projects/{}/alerts", f.slug);
    let (status, alerts_json) = f.get(&server, &alerts).await;
    assert_eq!(status, StatusCode::OK, "{alerts_json}");
    let alert = alerts_json
        .as_array()
        .and_then(|alerts| alerts.first())
        .expect("the outlier raised an alert");
    let model = model_uuid(&f.threshold, "model");
    let changepoint = fill_changepoint_model(&server, &f, uuid(&f.threshold)).await;

    let (status, body) = f.get(&server, &format!("{alerts}/{}", uuid(alert))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(model_uuid(&body["threshold"], "model"), model);
    assert_eq!(
        model_uuid(&body["threshold"], "changepoint_model"),
        Some(changepoint)
    );
}

// The console serves the changepoint slot beside the model, on the list row and on the threshold
// page with its own history, and serves an empty history for an empty slot.
#[tokio::test]
async fn the_console_serves_the_changepoint_slot() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "console").await;
    let console = format!(
        "/v0/console/projects/{}/thresholds/{}",
        f.slug,
        uuid(&f.threshold)
    );
    let (status, page) = f.get(&server, &console).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(page.get("changepoint_model"), None);
    assert_eq!(page.get("changepoint_models"), Some(&json!([])));
    assert_eq!(page["models"].as_array().map(Vec::len), Some(1));

    let changepoint = fill_changepoint_model(&server, &f, uuid(&f.threshold)).await;
    let (status, page) = f.get(&server, &console).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(model_uuid(&page, "changepoint_model"), Some(changepoint));
    assert_eq!(page["changepoint_model"]["test"], "changepoint");
    assert_eq!(page["changepoint_model"]["significance"], 0.001);

    let list = format!("/v0/console/projects/{}/thresholds", f.slug);
    let (status, list) = f.get(&server, &list).await;
    assert_eq!(status, StatusCode::OK, "{list}");
    let row = &list["thresholds"][0];
    assert_eq!(row["model"]["test"], "t_test");
    assert_eq!(row["changepoint_model"]["test"], "changepoint");
    assert_eq!(row["changepoint_model"]["significance"], 0.001);
}

// The threshold page's two histories each hold only their own lineage, newest first.
#[tokio::test]
async fn the_console_splits_the_model_histories_by_test() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "histories").await;
    let threshold = uuid(&f.threshold).to_owned();
    let first_model = model_uuid(&f.threshold, "model").expect("a model");
    let first_changepoint = fill_changepoint_model(&server, &f, &threshold).await;
    let second_changepoint = fill_changepoint_model_with(
        &server,
        &f,
        &threshold,
        &merge(changepoint_model(), &json!({ "max_sample_size": 25 })),
    )
    .await;
    let (status, body) = f
        .put(
            &server,
            &json!({ "test": "percentage", "upper_boundary": 0.1 }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let second_model = model_uuid(&body, "model").expect("a model");

    let console = format!("/v0/console/projects/{}/thresholds/{threshold}", f.slug);
    let (status, page) = f.get(&server, &console).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    let lineage = |history: &str| {
        page[history]
            .as_array()
            .expect("a history")
            .iter()
            .map(|model| {
                model["uuid"]
                    .as_str()
                    .and_then(|uuid| uuid.parse::<ModelUuid>().ok())
                    .expect("a model UUID")
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(lineage("models"), vec![second_model, first_model]);
    assert_eq!(
        lineage("changepoint_models"),
        vec![second_changepoint, first_changepoint]
    );
}

// A threshold takes a changepoint model in its changepoint slot on a create, and every threshold
// response shows it with its significance.
#[tokio::test]
async fn a_changepoint_model_fills_the_changepoint_slot() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "fills").await;
    let create = merge(
        new_threshold(Some("p99")),
        &json!({ "changepoint_model": changepoint_model() }),
    );
    let (status, created) = f.create(&server, &create).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(created["model"]["test"], "t_test");
    assert_eq!(
        parameters(&created["changepoint_model"]),
        changepoint_model()
    );
    let threshold = uuid(&created).to_owned();
    assert_eq!(
        slots(&server, &threshold).1,
        model_uuid(&created, "changepoint_model")
    );

    let (status, body) = f
        .get(&server, &format!("{}/{threshold}", f.thresholds()))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(parameters(&body["changepoint_model"]), changepoint_model());
    let (status, body) = f.get(&server, &f.thresholds()).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let listed = body
        .as_array()
        .and_then(|thresholds| thresholds.iter().find(|listed| uuid(listed) == threshold))
        .expect("the threshold is listed");
    assert_eq!(
        parameters(&listed["changepoint_model"]),
        changepoint_model()
    );
}

// A model whose test checks a line goes in `changepoint_model`, so `model` refuses it on a
// create and an update, and the refusal changes nothing.
#[tokio::test]
async fn a_line_test_in_the_model_slot_is_refused() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "line").await;
    let create = merge(
        json!({ "branch": "cp-branch", "testbed": "cp-testbed", "measure": "latency", "metric": "p99" }),
        &changepoint_model(),
    );
    let (status, body) = f.create(&server, &create).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.to_string().contains(LINE_TEST_REFUSAL), "{body}");

    let (status, body) = f.put(&server, &changepoint_model()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.to_string().contains(LINE_TEST_REFUSAL), "{body}");
    let (_, threshold) = f.get(&server, &f.threshold_path()).await;
    assert_eq!(threshold["model"]["uuid"], f.threshold["model"]["uuid"]);
    let (_, thresholds) = f.get(&server, &f.thresholds()).await;
    assert_eq!(thresholds.as_array().map(Vec::len), Some(1));
}

// Each refused changepoint model, and a significance on a metric test, is a 400 that names the
// field, on a create and an update alike.
#[tokio::test]
async fn a_refused_model_names_its_field() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "fields").await;
    let without = |field: &str| {
        let mut model = changepoint_model();
        if let Some(model) = model.as_object_mut() {
            model.remove(field);
        }
        model
    };
    let with = |field: &str, value: Value| merge(changepoint_model(), &json!({ field: value }));
    for (changepoint, field) in [
        (without("max_sample_size"), "max_sample_size"),
        (with("max_sample_size", json!(1001)), "max_sample_size"),
        (with("min_sample_size", json!(51)), "min_sample_size"),
        (without("upper_boundary"), "lower_boundary"),
        (with("upper_boundary", json!(-0.05)), "upper_boundary"),
        (without("significance"), "significance"),
        (with("significance", json!(1.0)), "significance"),
        (with("confirmations", json!(0)), "confirmations"),
        (with("confirmations", json!(21)), "confirmations"),
        (with("window", json!(86_400)), "window"),
    ] {
        let create = merge(
            new_threshold(Some("p99")),
            &json!({ "changepoint_model": changepoint }),
        );
        let update = merge(metric_model(), &json!({ "changepoint_model": changepoint }));
        for (status, body) in [
            f.create(&server, &create).await,
            f.put(&server, &update).await,
        ] {
            assert_eq!(status, StatusCode::BAD_REQUEST, "{field}: {body}");
            assert!(
                body.to_string().contains(&format!("`{field}`")),
                "{field}: {body}"
            );
        }
    }

    for (field, value) in [("significance", json!(0.001)), ("confirmations", json!(5))] {
        let refused = json!({ field: value });
        let create = merge(new_threshold(Some("p99")), &refused);
        let update = merge(metric_model(), &refused);
        for (status, body) in [
            f.create(&server, &create).await,
            f.put(&server, &update).await,
        ] {
            assert_eq!(status, StatusCode::BAD_REQUEST, "{field}: {body}");
            assert!(body.to_string().contains(&format!("`{field}`")), "{body}");
        }
    }
    let (_, thresholds) = f.get(&server, &f.thresholds()).await;
    assert_eq!(
        thresholds.as_array().map(Vec::len),
        Some(1),
        "no refused create made a threshold"
    );
    let (_, threshold) = f.get(&server, &f.threshold_path()).await;
    assert_eq!(threshold["model"]["uuid"], f.threshold["model"]["uuid"]);
    assert_eq!(threshold["changepoint_model"], Value::Null);
}

// A changepoint model that names no count of confirming scans stores 5, from a create, an update,
// and a report entry, and the same model sent again is no change.
#[tokio::test]
async fn a_changepoint_model_stores_5_confirmations_when_it_names_none() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "confirm5").await;
    let create = merge(
        new_threshold(Some("p99")),
        &json!({ "changepoint_model": changepoint_model_without_confirmations() }),
    );
    let (status, created) = f.create(&server, &create).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(
        created["changepoint_model"]["confirmations"], 5,
        "{created}"
    );

    let threshold = uuid(&f.threshold).to_owned();
    let update = merge(
        metric_model(),
        &json!({ "changepoint_model": changepoint_model_without_confirmations() }),
    );
    let (status, body) = f.put(&server, &update).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["changepoint_model"]["confirmations"], 5, "{body}");
    let filled = slots(&server, &threshold);
    for again in [
        update,
        merge(
            metric_model(),
            &json!({ "changepoint_model": changepoint_model() }),
        ),
    ] {
        let (status, body) = f.put(&server, &again).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(
            slots(&server, &threshold),
            filled,
            "the same model is no change"
        );
    }

    let entries = json!([
        { "measure": "latency", "metric": "p50", "model": changepoint_model_without_confirmations() },
    ]);
    post_report(&server, &f, 1, 1.0, 1, Some(json!({ "models": entries }))).await;
    assert_eq!(
        raw_rows(&mut server.db_conn(), STORED_CONFIRMATIONS),
        ["[50,5]", "[50,5]", "[50,5]"],
        "every changepoint model stores 5"
    );
}

/// Every stored count of confirming scans, with the test of its model.
const STORED_CONFIRMATIONS: &str = "SELECT json_array(test, confirmations) AS row FROM model WHERE confirmations IS NOT NULL ORDER BY id";

// A threshold can hold only a changepoint model: an update that removes its model and sets its
// changepoint model, or a report whose only entry for its key checks a line.
#[tokio::test]
async fn a_threshold_with_only_a_changepoint_model() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "only").await;
    let threshold = uuid(&f.threshold).to_owned();
    let (status, body) = f
        .put(
            &server,
            &json!({ "test": null, "changepoint_model": changepoint_model() }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["model"], Value::Null);
    assert_eq!(parameters(&body["changepoint_model"]), changepoint_model());
    let (model, changepoint) = slots(&server, &threshold);
    assert_eq!(model, None);
    assert_eq!(changepoint, model_uuid(&body, "changepoint_model"));

    let entries = json!([
        { "measure": "latency", "metric": "p50", "model": changepoint_model() },
    ]);
    post_report(&server, &f, 1, 1.0, 1, Some(json!({ "models": entries }))).await;
    let (_, thresholds) = f.get(&server, &f.thresholds()).await;
    let created = thresholds
        .as_array()
        .and_then(|thresholds| thresholds.iter().find(|listed| listed["metric"] == "p50"))
        .expect("the report created the threshold");
    assert_eq!(created["model"], Value::Null);
    assert_eq!(
        parameters(&created["changepoint_model"]),
        changepoint_model()
    );
}

// An update that names both slots checks the rate limit for both before it writes either, so a
// refusal on the second slot leaves the first unchanged.
#[cfg(feature = "plus")]
#[tokio::test]
async fn an_update_writes_both_slots_or_neither() {
    let server = TestServer::new_with_creation_limits(4, 4).await;
    let f = Fixture::new(&server, "atomic").await;
    let threshold = uuid(&f.threshold).to_owned();
    for upper_boundary in [0.1, 0.2] {
        let (status, body) = f
            .put(
                &server,
                &json!({ "test": "percentage", "upper_boundary": upper_boundary }),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }
    let before = slots(&server, &threshold);

    // The threshold has 3 models, so the limit of 4 holds one more but not two.
    let both = json!({
        "test": "percentage",
        "upper_boundary": 0.3,
        "changepoint_model": changepoint_model(),
    });
    let (status, body) = f.put(&server, &both).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    assert_eq!(
        slots(&server, &threshold),
        before,
        "the refusal changed no slot"
    );

    let (status, body) = f
        .put(
            &server,
            &json!({ "test": "percentage", "upper_boundary": 0.2, "changepoint_model": changepoint_model() }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "one write fits: {body}");
    assert_eq!(
        slots(&server, &threshold).0,
        before.0,
        "the model was no change"
    );
}

// At BMF version 1 an entry with a line test fills the changepoint slot, a metric entry and a
// changepoint entry for one key both apply, and within a slot the last entry wins.
#[tokio::test]
async fn a_v1_report_routes_each_entry_to_its_slot() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "routes").await;
    let threshold = uuid(&f.threshold).to_owned();
    let window = |max_sample_size: u32| {
        merge(
            changepoint_model(),
            &json!({ "max_sample_size": max_sample_size }),
        )
    };
    let entries = json!([
        { "measure": "latency", "metric": "value", "model": window(10) },
        { "measure": "latency", "metric": "value", "model": { "test": "percentage", "upper_boundary": 0.1 } },
        { "measure": "latency", "metric": "value", "model": window(20) },
    ]);
    post_report(&server, &f, 1, 1.0, 1, Some(json!({ "models": entries }))).await;
    let (_, body) = f.get(&server, &f.threshold_path()).await;
    assert_eq!(
        parameters(&body["model"]),
        json!({ "test": "percentage", "upper_boundary": 0.1 })
    );
    assert_eq!(parameters(&body["changepoint_model"]), window(20));
    let (model, changepoint) = slots(&server, &threshold);
    assert_eq!(model, model_uuid(&body, "model"));
    assert_eq!(changepoint, model_uuid(&body, "changepoint_model"));
    let (_, thresholds) = f.get(&server, &f.thresholds()).await;
    assert_eq!(
        thresholds.as_array().map(Vec::len),
        Some(1),
        "the entries are one threshold"
    );
}

// A report skips a changepoint entry that its test's rules refuse, with a warning, and goes on: the
// entry fills no slot and makes no threshold, and a valid entry beside it is stored. A metric entry
// is not checked against its test's rules.
#[tokio::test]
async fn a_v1_report_skips_an_invalid_changepoint_entry() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "skips").await;
    let bare = uuid(&f.threshold).to_owned();
    let mut no_significance = changepoint_model();
    no_significance
        .as_object_mut()
        .expect("a model")
        .remove("significance");
    let time_window = merge(changepoint_model(), &json!({ "window": 3600 }));
    // Sample sizes the t-test refuses.
    let unchecked = merge(
        metric_model(),
        &json!({ "min_sample_size": 10, "max_sample_size": 5 }),
    );

    for (day, invalid, valid) in [
        (
            1,
            json!({ "measure": "latency", "metric": "p50", "model": no_significance }),
            json!({ "measure": "latency", "metric": "p90", "model": changepoint_model() }),
        ),
        (
            2,
            json!({ "measure": "latency", "metric": "value", "model": time_window }),
            json!({ "measure": "latency", "metric": "value", "model": unchecked }),
        ),
    ] {
        let entries = json!([invalid, valid]);
        let report =
            post_report(&server, &f, day, 1.0, 1, Some(json!({ "models": entries }))).await;
        assert_eq!(
            report["warnings"],
            json!([{ "resource": "changepoint_model", "action": "skip", "count": 1 }]),
            "day {day}: {report}"
        );
    }

    let (_, body) = f.get(&server, &f.threshold_path()).await;
    assert_eq!(parameters(&body["model"]), unchecked);
    assert_eq!(body["changepoint_model"], Value::Null);
    assert_eq!(slots(&server, &bare).1, None);
    let (_, thresholds) = f.get(&server, &f.thresholds()).await;
    let thresholds = thresholds.as_array().expect("a list of thresholds");
    assert_eq!(
        thresholds
            .iter()
            .map(|threshold| threshold["metric"].as_str().unwrap_or("value"))
            .collect::<std::collections::BTreeSet<_>>(),
        ["p90", "value"].into(),
        "the invalid entry made no threshold: {thresholds:?}"
    );
    let p90 = thresholds
        .iter()
        .find(|threshold| threshold["metric"] == "p90")
        .expect("the valid entry made its threshold");
    assert_eq!(parameters(&p90["changepoint_model"]), changepoint_model());
}

/// Fields with a value their type refuses as it is read: a significance of at least 1, a
/// significance written as an integer, an empty sample, a boundary written as a negative
/// integer, and a count of confirming scans out of its range.
fn refused_values() -> [(&'static str, Value); 6] {
    [
        ("significance", json!(1.5)),
        ("significance", json!(1)),
        ("max_sample_size", json!(0)),
        ("upper_boundary", json!(-1)),
        ("confirmations", json!(0)),
        ("confirmations", json!(21)),
    ]
}

/// Whether a metric entry drops the field, whatever its value, instead of reading it.
fn metric_drops(field: &str) -> bool {
    ["significance", "confirmations"].contains(&field)
}

fn with_refused(model: Value, (field, value): &(&str, Value)) -> Value {
    merge(model, &json!({ *field: value }))
}

// A changepoint entry with a value its types refuse is skipped with the same warning, and the
// report goes on, while a metric entry with that value still fails the report, naming the field,
// unless the field is a significance or a count of confirming scans, which a metric entry drops.
#[tokio::test]
async fn a_v1_report_skips_a_changepoint_entry_its_types_refuse() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "types").await;
    let bare = uuid(&f.threshold).to_owned();
    let before = slots(&server, &bare);
    let entry = |model| json!([{ "measure": "latency", "metric": "value", "model": model }]);

    for (day, refused) in (1..).zip(refused_values()) {
        let changepoint = entry(with_refused(changepoint_model(), &refused));
        let report = post_report(
            &server,
            &f,
            day,
            1.0,
            1,
            Some(json!({ "models": changepoint })),
        )
        .await;
        assert_eq!(
            report["warnings"],
            json!([{ "resource": "changepoint_model", "action": "skip", "count": 1 }]),
            "{refused:?}: {report}"
        );
        assert_eq!(slots(&server, &bare), before, "{refused:?}");
    }
    for refused in refused_values()
        .into_iter()
        .filter(|(field, _)| !metric_drops(field))
    {
        let (field, _) = refused;
        let metric = entry(with_refused(metric_model(), &refused));
        let (status, body) =
            send_report(&server, &f, 1, 1.0, 1, Some(json!({ "models": metric }))).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{refused:?}: {body}");
        assert!(body.to_string().contains(field), "{refused:?}: {body}");
    }
    assert_eq!(
        raw_rows(&mut server.db_conn(), MODEL_TESTS),
        ["[0]"],
        "only the fixture's model is stored"
    );
}

// A reset report clears the changepoint slot of a threshold whose changepoint entry it skips, as
// it clears every slot the report does not name.
#[tokio::test]
async fn a_reset_clears_the_slot_of_a_skipped_changepoint_entry() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "skipreset").await;
    let bare = uuid(&f.threshold).to_owned();
    let changepoint = fill_changepoint_model(&server, &f, &bare).await;
    let (model, _) = slots(&server, &bare);

    let entries = json!([
        { "measure": "latency", "metric": "value", "model": metric_model() },
        {
            "measure": "latency",
            "metric": "value",
            "model": with_refused(changepoint_model(), &refused_values()[0]),
        },
    ]);
    let report = post_report(
        &server,
        &f,
        1,
        1.0,
        1,
        Some(json!({ "models": entries, "reset": true })),
    )
    .await;
    assert_eq!(
        report["warnings"],
        json!([{ "resource": "changepoint_model", "action": "skip", "count": 1 }]),
        "{report}"
    );
    assert_eq!(slots(&server, &bare), (model, None));
    assert!(
        is_replaced(&server, changepoint),
        "the reset replaced {changepoint}"
    );
}

// At BMF version 0 a map key names only the bare threshold's model, so a line test is refused.
#[tokio::test]
async fn a_v0_report_refuses_a_line_test() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "v0").await;
    let report = json!({
        "branch": "cp-branch",
        "testbed": "cp-testbed",
        "start_time": "2024-01-01T00:00:00Z",
        "end_time": "2024-01-01T00:01:00Z",
        "results": [json!({ "bench": { "latency": { "value": 1.0 } } }).to_string()],
        "bmf_version": 0,
        "thresholds": { "models": { "latency": changepoint_model() } },
    });
    let path = format!("/v0/projects/{}/reports", f.slug);
    let (status, body) = send(&server, &f.user, Method::POST, &path, Some(&report)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.to_string().contains(LINE_TEST_REFUSAL), "{body}");
    assert_eq!(slots(&server, uuid(&f.threshold)).1, None);
}

// A report drops the significance of a model whose test checks a metric, at BMF version 1 and 0,
// stores the rest of the model, and warns once for each significance it set aside.
#[tokio::test]
async fn a_report_drops_the_significance_of_a_metric_model() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "dropsig").await;
    let value = merge(metric_model(), &json!({ "upper_boundary": 0.97 }));
    let entries = json!([
        { "measure": "latency", "metric": "value", "model": with_significance(value.clone(), 0.5) },
        { "measure": "latency", "metric": "p50", "model": with_significance(metric_model(), 0.25) },
        { "measure": "latency", "metric": "p90", "model": changepoint_model() },
    ]);
    let report = post_report(&server, &f, 1, 1.0, 1, Some(json!({ "models": entries }))).await;
    assert_eq!(
        significance_warnings(&report),
        [json!({ "resource": "significance", "action": "ignore", "count": 2 })],
        "{report}"
    );
    let (_, body) = f.get(&server, &f.threshold_path()).await;
    assert_eq!(parameters(&body["model"]), value);
    let (_, thresholds) = f.get(&server, &f.thresholds()).await;
    let listed = |metric: &str| {
        thresholds
            .as_array()
            .and_then(|thresholds| thresholds.iter().find(|listed| listed["metric"] == metric))
            .cloned()
            .expect("the report made the threshold")
    };
    assert_eq!(parameters(&listed("p50")["model"]), metric_model());
    assert_eq!(
        parameters(&listed("p90")["changepoint_model"]),
        changepoint_model()
    );

    let v0 = merge(metric_model(), &json!({ "upper_boundary": 0.96 }));
    let models = json!({ "latency": with_significance(v0.clone(), 0.25) });
    let report = post_report(&server, &f, 2, 1.0, 0, Some(json!({ "models": models }))).await;
    assert_eq!(
        significance_warnings(&report),
        [json!({ "resource": "significance", "action": "ignore", "count": 1 })],
        "{report}"
    );
    let (_, body) = f.get(&server, &f.threshold_path()).await;
    assert_eq!(parameters(&body["model"]), v0);
    assert_eq!(
        raw_rows(&mut server.db_conn(), STORED_SIGNIFICANCES),
        ["[50,0.001]"],
        "only the changepoint model has a significance"
    );
}

// A report drops a significance of any value from a model whose test checks a metric, at BMF
// version 1 and 0, even one its type refuses, with one warning, and stores the rest of the model.
#[tokio::test]
async fn a_report_drops_any_significance_of_a_metric_model() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "anysig").await;
    let values = [json!(0.5), json!(1.5), json!(1), json!("0.01"), Value::Null];
    let mut day = 0;
    for bmf_version in [1, 0] {
        for (upper_boundary, significance) in
            [0.91, 0.92, 0.93, 0.94, 0.95].into_iter().zip(&values)
        {
            day += 1;
            let model = merge(metric_model(), &json!({ "upper_boundary": upper_boundary }));
            let written = merge(model.clone(), &json!({ "significance": significance }));
            let models = if bmf_version == 0 {
                json!({ "latency": written })
            } else {
                json!([{ "measure": "latency", "metric": "value", "model": written }])
            };
            let report = post_report(
                &server,
                &f,
                day,
                1.0,
                bmf_version,
                Some(json!({ "models": models })),
            )
            .await;
            let case = format!("version {bmf_version}, significance {significance}");
            assert_eq!(
                report["warnings"],
                json!([{ "resource": "significance", "action": "ignore", "count": 1 }]),
                "{case}: {report}"
            );
            let (_, body) = f.get(&server, &f.threshold_path()).await;
            assert_eq!(parameters(&body["model"]), model, "{case}");
        }
    }
    assert!(
        raw_rows(&mut server.db_conn(), STORED_SIGNIFICANCES).is_empty(),
        "no model has a significance"
    );
}

// A report drops a count of confirming scans of any value from a model whose test checks a
// metric, at BMF version 1 and 0, with one warning for each it set aside, and stores the rest of
// the model.
#[tokio::test]
async fn a_report_drops_the_confirmations_of_a_metric_model() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "dropconf").await;
    let values = [json!(5), json!(21), json!("5"), Value::Null];
    let mut day = 0;
    for bmf_version in [1, 0] {
        for (upper_boundary, confirmations) in [0.91, 0.92, 0.93, 0.94].into_iter().zip(&values) {
            day += 1;
            let model = merge(metric_model(), &json!({ "upper_boundary": upper_boundary }));
            let written = merge(model.clone(), &json!({ "confirmations": confirmations }));
            let models = if bmf_version == 0 {
                json!({ "latency": written })
            } else {
                json!([
                    { "measure": "latency", "metric": "value", "model": written },
                    { "measure": "latency", "metric": "p50", "model": written },
                ])
            };
            let report = post_report(
                &server,
                &f,
                day,
                1.0,
                bmf_version,
                Some(json!({ "models": models })),
            )
            .await;
            let case = format!("version {bmf_version}, confirmations {confirmations}");
            let count = if bmf_version == 0 { 1 } else { 2 };
            assert_eq!(
                confirmations_warnings(&report),
                [json!({ "resource": "confirmations", "action": "ignore", "count": count })],
                "{case}: {report}"
            );
            let (_, body) = f.get(&server, &f.threshold_path()).await;
            assert_eq!(parameters(&body["model"]), model, "{case}");
        }
    }
    assert!(
        raw_rows(&mut server.db_conn(), STORED_CONFIRMATIONS).is_empty(),
        "no model has a count of confirming scans"
    );
}

/// The report's warnings of a count of confirming scans it set aside.
fn confirmations_warnings(report: &Value) -> Vec<Value> {
    report["warnings"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|warning| warning["resource"] == "confirmations")
        .cloned()
        .collect()
}

// A report refuses a key written twice in a model only for a field the model reads strictly, at
// BMF version 1 and 0: the last `significance` and the last unknown key win.
#[tokio::test]
async fn a_report_reads_the_last_of_a_key_a_model_may_set_aside() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "dupkey").await;
    let ignored = json!([{ "resource": "significance", "action": "ignore", "count": 1 }]);
    let ignored_confirmations =
        json!([{ "resource": "confirmations", "action": "ignore", "count": 1 }]);
    let mut day = 0;
    for bmf_version in [1, 0] {
        for (upper_boundary, twice, warnings) in [
            (0.91, r#""significance":0.1,"significance":0.2"#, &ignored),
            (0.92, r#""note":1,"note":2"#, &Value::Null),
            (
                0.93,
                r#""confirmations":1,"confirmations":2"#,
                &ignored_confirmations,
            ),
        ] {
            day += 1;
            let model = merge(metric_model(), &json!({ "upper_boundary": upper_boundary }));
            let case = format!("version {bmf_version}, {twice}");
            let (status, report) =
                send_raw_report(&server, &f, day, bmf_version, &with_keys(&model, twice)).await;
            assert_eq!(status, StatusCode::CREATED, "{case}: {report}");
            assert_eq!(&report["warnings"], warnings, "{case}: {report}");
            let (_, body) = f.get(&server, &f.threshold_path()).await;
            assert_eq!(parameters(&body["model"]), model, "{case}");
        }

        day += 1;
        let twice = with_keys(&metric_model(), r#""upper_boundary":0.97"#);
        let (status, body) = send_raw_report(&server, &f, day, bmf_version, &twice).await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "version {bmf_version}: {body}"
        );
        assert!(
            body.to_string()
                .contains("duplicate field `upper_boundary`"),
            "version {bmf_version}: {body}"
        );
    }

    day += 1;
    let first = merge(changepoint_model(), &json!({ "significance": 0.5 }));
    let twice = with_keys(&first, r#""significance":0.001"#);
    let (status, report) = send_raw_report(&server, &f, day, 1, &twice).await;
    assert_eq!(status, StatusCode::CREATED, "{report}");
    assert_eq!(report["warnings"], Value::Null, "{report}");
    let (_, body) = f.get(&server, &f.threshold_path()).await;
    assert_eq!(parameters(&body["changepoint_model"]), changepoint_model());
}

/// `model` as raw JSON with `keys` written after its own, so a key may appear twice.
fn with_keys(model: &Value, keys: &str) -> String {
    let model = model.to_string();
    let fields = model.strip_suffix('}').expect("a JSON object");
    format!("{fields},{keys}}}")
}

/// Sends a report that declares one model for the fixture's bare threshold, written as raw JSON.
async fn send_raw_report(
    server: &TestServer,
    f: &Fixture,
    day: usize,
    bmf_version: u8,
    model: &str,
) -> (StatusCode, Value) {
    let models = if bmf_version == 0 {
        json!({ "latency": "MODEL" })
    } else {
        json!([{ "measure": "latency", "metric": "value", "model": "MODEL" }])
    };
    let report = report_body(day, 1.0, bmf_version, Some(&json!({ "models": models })))
        .to_string()
        .replace("\"MODEL\"", model);
    let path = format!("/v0/projects/{}/reports", f.slug);
    let resp = server
        .client
        .post(server.api_url(&path))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&f.user.token),
        )
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(report)
        .send()
        .await
        .expect("Request failed");
    let status = resp.status();
    let text = resp.text().await.expect("Failed to read the response");
    (
        status,
        serde_json::from_str(&text).unwrap_or(Value::String(text)),
    )
}

// A BMF version 0 report reads a model written as an array of its fields in order, and drops the
// significance and the count of confirming scans of one whose test checks a metric with the same
// warnings.
#[tokio::test]
async fn a_v0_report_reads_a_model_written_as_an_array() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "v0array").await;
    let ignored = json!([{ "resource": "significance", "action": "ignore", "count": 1 }]);
    let both = json!([
        { "resource": "significance", "action": "ignore", "count": 1 },
        { "resource": "confirmations", "action": "ignore", "count": 1 },
    ]);
    for (day, written, warnings) in [
        (1, json!(["t_test", 2, 64, null, null, 0.97]), Value::Null),
        (2, json!(["t_test", 2, 64, null, null, 0.96, 0.5]), ignored),
        (3, json!(["t_test", 2, 64, null, null, 0.95, null, 5]), both),
    ] {
        let models = json!({ "latency": written });
        let report = post_report(&server, &f, day, 1.0, 0, Some(json!({ "models": models }))).await;
        assert_eq!(report["warnings"], warnings, "{written}: {report}");
        let (_, body) = f.get(&server, &f.threshold_path()).await;
        let model = merge(metric_model(), &json!({ "upper_boundary": written[5] }));
        assert_eq!(parameters(&body["model"]), model, "{written}");
    }
    assert!(
        raw_rows(&mut server.db_conn(), STORED_SIGNIFICANCES).is_empty(),
        "no model has a significance"
    );
    assert!(
        raw_rows(&mut server.db_conn(), STORED_CONFIRMATIONS).is_empty(),
        "no model has a count of confirming scans"
    );
}

/// Every stored significance, with the test of its model.
const STORED_SIGNIFICANCES: &str = "SELECT json_array(test, significance) AS row FROM model WHERE significance IS NOT NULL ORDER BY id";

fn with_significance(model: Value, significance: f64) -> Value {
    merge(model, &json!({ "significance": significance }))
}

/// The report's warnings of a significance it set aside.
fn significance_warnings(report: &Value) -> Vec<Value> {
    report["warnings"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|warning| warning["resource"] == "significance")
        .cloned()
        .collect()
}

/// One row per model whose test checks a metric: every column a model had before `significance`.
const METRIC_MODEL_ROWS: &str = "SELECT json_array(id, uuid, threshold_id, test, min_sample_size, max_sample_size, window, lower_boundary, upper_boundary, created, replaced) AS row FROM model WHERE test != 50 ORDER BY id";
/// The columns of `model`, in order.
const MODEL_COLUMNS: &str = "SELECT name AS row FROM pragma_table_info('model') ORDER BY cid";
/// Every foreign key of `model`.
const MODEL_KEYS: &str = "SELECT json_array(\"table\", \"from\", \"to\", on_update, on_delete) AS row FROM pragma_foreign_key_list('model') ORDER BY row";
/// Every index on `model`, as written, its unique UUID included.
const MODEL_INDEXES: &str = "SELECT json_array(name, sql) AS row FROM sqlite_master WHERE tbl_name = 'model' AND type = 'index' ORDER BY name";
/// Every model's test.
const MODEL_TESTS: &str = "SELECT json_array(test) AS row FROM model ORDER BY id";
/// Every model's test and significance.
const SIGNIFICANCES: &str = "SELECT json_array(test, significance) AS row FROM model ORDER BY id";
/// Every report warning: its resource, action, and count.
const WARNINGS: &str = "SELECT json_array(resource, action, count) AS row FROM report_warning ORDER BY resource, action";

// The migration down and up keeps every model that checks a metric, column for column, with every
// key and index. Down takes each changepoint model and empties its slot, since no test can read
// it, and each report warning whose resource older code cannot read, and up puts `significance`
// right after `upper_boundary`, empty on every row.
// The server keeps serving on its own threads while this test's connection holds a lock.
#[tokio::test(flavor = "multi_thread")]
async fn significance_migration_down_and_up_keeps_every_metric_model() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "significance").await;
    let create = merge(
        new_threshold(Some("p99")),
        &json!({ "changepoint_model": changepoint_model() }),
    );
    let (status, body) = f.create(&server, &create).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let entries = json!([
        {
            "measure": "latency",
            "metric": "p50",
            "model": with_refused(changepoint_model(), &refused_values()[0]),
        },
        {
            "measure": "latency",
            "metric": "p99",
            "model": merge(with_significance(metric_model(), 0.5), &json!({ "confirmations": 5 })),
        },
    ]);
    post_report(&server, &f, 1, 1.0, 1, Some(json!({ "models": entries }))).await;

    let mut conn = server.db_conn();
    // The results lack the p99 metric, so its threshold is skipped: a warning older code reads.
    assert_eq!(
        raw_rows(&mut conn, WARNINGS),
        ["[4,0,1]", "[7,0,1]", "[8,1,1]", "[9,1,1]"],
        "a threshold, a changepoint model, a significance, and a confirmations warning"
    );
    let models = raw_rows(&mut conn, METRIC_MODEL_ROWS);
    let columns = raw_rows(&mut conn, MODEL_COLUMNS);
    let keys = raw_rows(&mut conn, MODEL_KEYS);
    let indexes = raw_rows(&mut conn, MODEL_INDEXES);
    assert_eq!(models.len(), 2, "{models:?}");
    assert_eq!(
        raw_rows(&mut conn, SIGNIFICANCES),
        ["[0,null]", "[0,null]", "[50,0.001]"],
        "only the changepoint model has a significance"
    );
    let upper_boundary = columns
        .iter()
        .position(|column| column == "upper_boundary")
        .expect("an upper boundary column");
    assert_eq!(
        columns
            .get(upper_boundary + 1..=upper_boundary + 2)
            .map(<[String]>::to_vec),
        Some(vec!["significance".to_owned(), "confirmations".to_owned()]),
        "the significance and the count of confirming scans sit right after the upper boundary"
    );

    revert_migration(&mut conn, SIGNIFICANCE_MIGRATION);
    assert_eq!(
        raw_rows(&mut conn, METRIC_MODEL_ROWS),
        models,
        "down keeps every metric model"
    );
    assert_eq!(
        raw_rows(&mut conn, MODEL_TESTS),
        ["[0]", "[0]"],
        "down takes the changepoint model"
    );
    let without_significance = columns
        .iter()
        .filter(|column| !["significance", "confirmations"].contains(&column.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(raw_rows(&mut conn, MODEL_COLUMNS), without_significance);
    assert_eq!(
        raw_rows(&mut conn, MODEL_KEYS),
        keys,
        "down keeps every key"
    );
    assert_eq!(
        raw_rows(&mut conn, MODEL_INDEXES),
        indexes,
        "down keeps every index"
    );
    assert!(
        raw_rows(&mut conn, CHANGEPOINT_SLOTS)
            .iter()
            .all(|slot| slot.ends_with(",null]")),
        "down empties every changepoint slot"
    );
    assert_eq!(
        raw_rows(&mut conn, BROKEN_KEYS),
        Vec::<String>::new(),
        "down breaks no key"
    );
    assert_eq!(
        raw_rows(&mut conn, WARNINGS),
        ["[4,0,1]"],
        "down takes each warning older code cannot read"
    );

    apply_migration(&mut conn);
    assert_eq!(
        raw_rows(&mut conn, METRIC_MODEL_ROWS),
        models,
        "up keeps every metric model"
    );
    assert_eq!(raw_rows(&mut conn, WARNINGS), ["[4,0,1]"]);
    assert_eq!(
        raw_rows(&mut conn, MODEL_COLUMNS),
        columns,
        "up restores the column"
    );
    assert_eq!(
        raw_rows(&mut conn, MODEL_KEYS),
        keys,
        "up restores every key"
    );
    assert_eq!(
        raw_rows(&mut conn, MODEL_INDEXES),
        indexes,
        "up restores every index"
    );
    assert_eq!(
        raw_rows(&mut conn, SIGNIFICANCES),
        ["[0,null]", "[0,null]"],
        "every significance comes up empty"
    );
    assert_eq!(
        raw_rows(&mut conn, BROKEN_KEYS),
        Vec::<String>::new(),
        "up breaks no key"
    );
    drop(conn);

    let (status, threshold) = f.get(&server, &f.threshold_path()).await;
    assert_eq!(status, StatusCode::OK, "{threshold}");
    assert_eq!(threshold["model"]["uuid"], f.threshold["model"]["uuid"]);
}
