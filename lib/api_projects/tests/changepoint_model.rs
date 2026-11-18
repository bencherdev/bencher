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
use bencher_json::{DateTime, Model, ModelUuid, ThresholdUuid};
use bencher_schema::{
    MIGRATIONS,
    context::DbConnection,
    model::project::threshold::{
        ThresholdId,
        model::{InsertModel, ModelId, UpdateModel},
    },
    schema,
};
use diesel::{
    ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _, connection::SimpleConnection as _,
};
use diesel_migrations::MigrationHarness as _;
use http::{Method, StatusCode};
use serde_json::{Value, json};

const CHANGEPOINT_MODEL_MIGRATION: &str = "20261118120000";

/// The refusal of a metric test in the changepoint slot.
const METRIC_TEST_REFUSAL: &str =
    "checks a metric, so it belongs in `model`, not in `changepoint_model`";

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

    revert_migration(&mut conn);
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

/// Revert every migration down to and including the changepoint model migration.
fn revert_migration(conn: &mut DbConnection) {
    conn.batch_execute("PRAGMA foreign_keys = OFF")
        .expect("Failed to disable foreign keys");
    loop {
        let version = conn
            .revert_last_migration(MIGRATIONS)
            .expect("Failed to revert a migration");
        if version.to_string() == CHANGEPOINT_MODEL_MIGRATION {
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

/// No API can fill the changepoint slot until a test checks a line, so this goes straight into
/// the database: a model row of the threshold, held in its changepoint slot.
fn seed_changepoint_model(server: &TestServer, threshold: &str) -> ModelUuid {
    seed_changepoint_model_with(
        server,
        threshold,
        &json!({
            "test": "percentage",
            "max_sample_size": 50,
            "upper_boundary": 0.05,
        }),
    )
}

/// Fill the changepoint slot with a model of these parameters, replacing the model it held.
fn seed_changepoint_model_with(server: &TestServer, threshold: &str, model: &Value) -> ModelUuid {
    let threshold_id = threshold_id(server, threshold);
    let model: Model = serde_json::from_value(model.clone()).expect("a model");
    let insert_model = InsertModel::new(threshold_id, model);
    let mut conn = server.db_conn();
    let held: Option<ModelId> = schema::threshold::table
        .filter(schema::threshold::id.eq(threshold_id))
        .select(schema::threshold::changepoint_model_id)
        .first(&mut conn)
        .expect("Failed to load the changepoint slot");
    if let Some(held) = held {
        diesel::update(schema::model::table.filter(schema::model::id.eq(held)))
            .set(&UpdateModel::replaced_at(DateTime::now()))
            .execute(&mut conn)
            .expect("Failed to replace the held model");
    }
    diesel::insert_into(schema::model::table)
        .values(&insert_model)
        .execute(&mut conn)
        .expect("Failed to insert the model");
    let model_id: ModelId = schema::model::table
        .filter(schema::model::uuid.eq(insert_model.uuid))
        .select(schema::model::id)
        .first(&mut conn)
        .expect("Failed to load the model id");
    diesel::update(schema::threshold::table.filter(schema::threshold::id.eq(threshold_id)))
        .set(schema::threshold::changepoint_model_id.eq(model_id))
        .execute(&mut conn)
        .expect("Failed to fill the changepoint slot");
    insert_model.uuid
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
    let changepoint = seed_changepoint_model(&server, &threshold);
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
    let measures = json!({ "latency": { "value": value } });
    let results = if bmf_version == 0 {
        json!({ "bench": measures })
    } else {
        json!({ "bench": [{ "parameters": {}, "measures": measures }] })
    };
    let report = json!({
        "branch": "cp-branch",
        "testbed": "cp-testbed",
        "start_time": format!("2024-01-{day:02}T00:00:00Z"),
        "end_time": format!("2024-01-{day:02}T00:01:00Z"),
        "results": [results.to_string()],
        "bmf_version": bmf_version,
        "thresholds": thresholds,
    });
    let path = format!("/v0/projects/{}/reports", f.slug);
    let (status, body) = send(server, &f.user, Method::POST, &path, Some(&report)).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body
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
    let bare_changepoint = seed_changepoint_model(&server, &bare);
    let named_changepoint = seed_changepoint_model(&server, &named);
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

/// What a model checks with, without the row it is.
fn parameters(model: &Value) -> Value {
    let mut model = model.clone();
    if let Some(model) = model.as_object_mut() {
        for row in ["uuid", "created", "replaced"] {
            model.remove(row);
        }
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
    let changepoint = seed_changepoint_model(&server, &source);
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
    seed_changepoint_model(&server, &source);

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

    let source_changepoint = seed_changepoint_model_with(
        &server,
        &source,
        &json!({
            "test": "percentage",
            "max_sample_size": 25,
            "upper_boundary": 0.2,
        }),
    );
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
    seed_changepoint_model(&server, &threshold);

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
    let changepoint = seed_changepoint_model(&server, &threshold);

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
    let changepoint = seed_changepoint_model(&server, &threshold);
    let first = model_uuid(&f.threshold, "model").expect("a model");
    let (status, body) = f
        .put(
            &server,
            &json!({ "test": "percentage", "upper_boundary": 0.1 }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, other) = f.create(&server, &new_threshold(Some("p99"))).await;
    assert_eq!(status, StatusCode::CREATED, "{other}");
    let other = model_uuid(&other, "model").expect("a model");

    let path = f.threshold_path();
    let (status, body) = f.get(&server, &format!("{path}?model={first}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(model_uuid(&body, "model"), Some(first));
    assert_eq!(model_uuid(&body, "changepoint_model"), Some(changepoint));

    // The seeded changepoint model's test checks a metric, so it is not pinned in its slot.
    for query in [
        format!("changepoint_model={changepoint}"),
        format!("changepoint_model={first}"),
        format!("model={first}&changepoint_model={changepoint}"),
    ] {
        let (status, body) = f.get(&server, &format!("{path}?{query}")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{query}: {body}");
        assert!(
            body.to_string().contains(METRIC_TEST_REFUSAL),
            "{query}: {body}"
        );
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
    let changepoint = seed_changepoint_model(&server, uuid(&f.threshold));

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

    let changepoint = seed_changepoint_model(&server, uuid(&f.threshold));
    let (status, page) = f.get(&server, &console).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(model_uuid(&page, "changepoint_model"), Some(changepoint));
    assert_eq!(page["changepoint_model"]["test"], "percentage");

    let list = format!("/v0/console/projects/{}/thresholds", f.slug);
    let (status, list) = f.get(&server, &list).await;
    assert_eq!(status, StatusCode::OK, "{list}");
    let row = &list["thresholds"][0];
    assert_eq!(row["model"]["test"], "t_test");
    assert_eq!(row["changepoint_model"]["test"], "percentage");
}
