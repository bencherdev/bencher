#![cfg(feature = "plus")]
#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::struct_field_names,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! The changepoints of a project: the list, the view, and the status change.

use bencher_api_tests::{
    TestServer, TestUser,
    helpers::{base_timestamp, get_empty_variant, get_project_id, grant_project_role},
};
use bencher_json::{
    ChangepointUuid, DateTime, HeadIdempotencyKey, JsonChangepoint, JsonChangepoints,
    JsonProjectKeyCreated, SpecUuid,
};
use bencher_rbac::project::Role;
use bencher_schema::{
    MIGRATIONS, context::DbConnection, model::project::threshold::changepoint::ChangepointState,
    schema,
};
use diesel::{
    ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _, connection::SimpleConnection as _,
};
use diesel_migrations::MigrationHarness as _;
use http::{Method, StatusCode};
use serde_json::{Value, json};

const CHANGEPOINT_MIGRATION: &str = "20261125120000";

/// A project with two branches, two testbeds, two measures, and two benchmarks, a threshold with a
/// changepoint model on each line the tests use, and a report on each of those lines.
struct Fixture {
    user: TestUser,
    slug: String,
}

impl Fixture {
    async fn new(server: &TestServer, label: &str) -> Self {
        let user = server
            .signup("Test User", &format!("changepoints{label}@example.com"))
            .await;
        let org = server
            .create_org(&user, &format!("Changepoint Org {label}"))
            .await;
        let project = server
            .create_project(&user, &org, &format!("Changepoint Project {label}"))
            .await;
        let f = Self {
            user,
            slug: project.slug.to_string(),
        };
        for (resource, body) in [
            ("branches", json!({ "name": "main-branch" })),
            ("branches", json!({ "name": "other-branch" })),
            ("testbeds", json!({ "name": "main-testbed" })),
            ("testbeds", json!({ "name": "other-testbed" })),
            (
                "measures",
                json!({ "name": "latency", "units": "nanoseconds" }),
            ),
            (
                "measures",
                json!({ "name": "throughput", "units": "operations" }),
            ),
            ("measures", json!({ "name": "size", "units": "bytes" })),
        ] {
            let (status, body) = f.send(server, Method::POST, resource, Some(&body)).await;
            assert_eq!(status, StatusCode::CREATED, "{resource}: {body}");
        }
        for (branch, testbed, measure) in [
            ("main-branch", "main-testbed", "latency"),
            ("other-branch", "main-testbed", "latency"),
            ("main-branch", "other-testbed", "latency"),
            ("main-branch", "main-testbed", "throughput"),
            ("main-branch", "main-testbed", "size"),
        ] {
            let threshold = json!({
                "branch": branch,
                "testbed": testbed,
                "measure": measure,
                "test": "t_test",
                "upper_boundary": 0.99,
                "changepoint_model": {
                    "test": "changepoint",
                    "max_sample_size": 50,
                    "upper_boundary": 0.05,
                    "significance": 0.001,
                },
            });
            let (status, body) = f
                .send(server, Method::POST, "thresholds", Some(&threshold))
                .await;
            assert_eq!(status, StatusCode::CREATED, "{body}");
        }
        for (day, branch, testbed) in [
            (1, "main-branch", "main-testbed"),
            (2, "other-branch", "main-testbed"),
            (3, "main-branch", "other-testbed"),
        ] {
            let measures = json!({ "latency": { "value": 1.0 }, "throughput": { "value": 2.0 } });
            let report = json!({
                "branch": branch,
                "testbed": testbed,
                "start_time": format!("2024-01-{day:02}T00:00:00Z"),
                "end_time": format!("2024-01-{day:02}T00:01:00Z"),
                "results": [json!({ "bench-one": measures, "bench-two": measures }).to_string()],
            });
            let (status, body) = f.send(server, Method::POST, "reports", Some(&report)).await;
            assert_eq!(status, StatusCode::CREATED, "{body}");
        }
        f
    }

    async fn send(
        &self,
        server: &TestServer,
        method: Method,
        resource: &str,
        body: Option<&Value>,
    ) -> (StatusCode, Value) {
        send(server, &self.user.token, method, &self.path(resource), body).await
    }

    fn path(&self, resource: &str) -> String {
        if resource.starts_with('?') {
            format!("/v0/projects/{}{resource}", self.slug)
        } else {
            format!("/v0/projects/{}/{resource}", self.slug)
        }
    }

    /// The identifiers of the line on the branch's current head, at the newest version a report
    /// on the testbed is on.
    fn line(&self, server: &TestServer, line: Line) -> LineIds {
        let Line {
            branch,
            testbed,
            benchmark,
            measure,
        } = line;
        let mut conn = server.db_conn();
        let project_id = get_project_id(server, &self.slug);
        let (branch_id, head_id) = schema::branch::table
            .filter(schema::branch::project_id.eq(project_id))
            .filter(schema::branch::slug.eq(branch))
            .select((schema::branch::id, schema::branch::head_id))
            .first::<(i32, Option<i32>)>(&mut conn)
            .expect("Failed to get the branch");
        let head_id = head_id.expect("a head");
        let (testbed_id, spec_id) = schema::testbed::table
            .filter(schema::testbed::project_id.eq(project_id))
            .filter(schema::testbed::slug.eq(testbed))
            .select((schema::testbed::id, schema::testbed::spec_id))
            .first::<(i32, Option<i32>)>(&mut conn)
            .expect("Failed to get the testbed");
        let benchmark_id = schema::benchmark::table
            .filter(schema::benchmark::project_id.eq(project_id))
            .filter(schema::benchmark::slug.eq(benchmark))
            .select(schema::benchmark::id)
            .first::<i32>(&mut conn)
            .expect("Failed to get the benchmark");
        let variant_id = get_empty_variant(&mut conn, benchmark_id);
        let measure_id = schema::measure::table
            .filter(schema::measure::project_id.eq(project_id))
            .filter(schema::measure::slug.eq(measure))
            .select(schema::measure::id)
            .first::<i32>(&mut conn)
            .expect("Failed to get the measure");
        let (threshold_id, model_id) = schema::threshold::table
            .filter(schema::threshold::branch_id.eq(branch_id))
            .filter(schema::threshold::testbed_id.eq(testbed_id))
            .filter(schema::threshold::measure_id.eq(measure_id))
            .select((
                schema::threshold::id,
                schema::threshold::changepoint_model_id,
            ))
            .first::<(i32, Option<i32>)>(&mut conn)
            .expect("Failed to get the threshold");
        let version_id = schema::report::table
            .inner_join(schema::version::table)
            .filter(schema::report::head_id.eq(head_id))
            .filter(schema::report::testbed_id.eq(testbed_id))
            .order(schema::version::number.desc())
            .select(schema::report::version_id)
            .first::<i32>(&mut conn)
            .ok();
        LineIds {
            project_id,
            head_id,
            testbed_id,
            spec_id,
            benchmark_id,
            variant_id,
            measure_id,
            version_id,
            threshold_id,
            model_id: model_id.expect("a changepoint model"),
        }
    }
}

#[derive(Clone, Copy)]
struct Line {
    branch: &'static str,
    testbed: &'static str,
    benchmark: &'static str,
    measure: &'static str,
}

const MAIN: Line = Line {
    branch: "main-branch",
    testbed: "main-testbed",
    benchmark: "bench-one",
    measure: "latency",
};

struct LineIds {
    project_id: i32,
    head_id: i32,
    testbed_id: i32,
    /// The testbed's spec when the line is read.
    spec_id: Option<i32>,
    benchmark_id: i32,
    variant_id: i32,
    measure_id: i32,
    version_id: Option<i32>,
    threshold_id: i32,
    model_id: i32,
}

/// A changepoint's means, standard deviations, and t statistic.
#[derive(Clone, Copy)]
struct Measured {
    mean_before: f64,
    mean_after: f64,
    tstatistic: Option<f64>,
}

const STEP: Measured = Measured {
    mean_before: 100.0,
    mean_after: 110.0,
    tstatistic: Some(-21.5),
};

/// Insert a changepoint on the line in this state, created and modified at the given offsets in
/// seconds from the base time.
fn insert(
    server: &TestServer,
    line: &LineIds,
    state: ChangepointState,
    measured: Measured,
    created: i64,
    modified: i64,
) -> ChangepointUuid {
    let uuid = ChangepointUuid::new();
    let Measured {
        mean_before,
        mean_after,
        tstatistic,
    } = measured;
    diesel::insert_into(schema::changepoint::table)
        .values((
            schema::changepoint::uuid.eq(uuid),
            schema::changepoint::project_id.eq(line.project_id),
            schema::changepoint::head_id.eq(line.head_id),
            schema::changepoint::testbed_id.eq(line.testbed_id),
            schema::changepoint::spec_id.eq(line.spec_id),
            schema::changepoint::benchmark_id.eq(line.benchmark_id),
            schema::changepoint::variant_id.eq(line.variant_id),
            schema::changepoint::measure_id.eq(line.measure_id),
            schema::changepoint::metric.eq("value"),
            schema::changepoint::version_id.eq(line.version_id.expect("a version")),
            schema::changepoint::threshold_id.eq(line.threshold_id),
            schema::changepoint::model_id.eq(line.model_id),
            schema::changepoint::mean_before.eq(mean_before),
            schema::changepoint::mean_after.eq(mean_after),
            schema::changepoint::stddev_before.eq(2.0),
            schema::changepoint::stddev_after.eq(2.5),
            schema::changepoint::pvalue.eq(0.000_1),
            schema::changepoint::tstatistic.eq(tstatistic),
            schema::changepoint::degrees_of_freedom.eq(98),
            schema::changepoint::status.eq(state),
            schema::changepoint::created.eq(seconds(created)),
            schema::changepoint::modified.eq(seconds(modified)),
        ))
        .execute(&mut server.db_conn())
        .expect("Failed to insert a changepoint");
    uuid
}

/// Insert a scan of the changepoint that measured the step's mean after as `mean_after`, created
/// at the given offset in seconds from the base time.
fn insert_scan(server: &TestServer, uuid: ChangepointUuid, mean_after: f64, created: i64) {
    let changepoint_id = changepoint_id(server, uuid);
    let mut conn = server.db_conn();
    diesel::insert_into(schema::changepoint_scan::table)
        .values((
            schema::changepoint_scan::changepoint_id.eq(changepoint_id),
            schema::changepoint_scan::mean_before.eq(100.0),
            schema::changepoint_scan::mean_after.eq(mean_after),
            schema::changepoint_scan::stddev_before.eq(2.0),
            schema::changepoint_scan::stddev_after.eq(2.5),
            schema::changepoint_scan::pvalue.eq(0.000_1),
            schema::changepoint_scan::tstatistic.eq(None::<f64>),
            schema::changepoint_scan::degrees_of_freedom.eq(98),
            schema::changepoint_scan::created.eq(seconds(created)),
        ))
        .execute(&mut conn)
        .expect("Failed to insert a scan");
}

fn seconds(offset: i64) -> DateTime {
    DateTime::try_from(base_timestamp().timestamp() + offset).expect("a time")
}

async fn send(
    server: &TestServer,
    token: &str,
    method: Method,
    path: &str,
    body: Option<&Value>,
) -> (StatusCode, Value) {
    let mut request = server.client.request(method, server.api_url(path)).header(
        bencher_json::AUTHORIZATION,
        bencher_json::bearer_header(token),
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

/// The changepoints a list returns, by UUID in order, and its total count.
async fn list(server: &TestServer, f: &Fixture, query: &str) -> (Vec<String>, u64) {
    let resp = server
        .client
        .get(server.api_url(&format!("{}{query}", f.path("changepoints"))))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&f.user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK, "{query}");
    let total = resp
        .headers()
        .get("X-Total-Count")
        .and_then(|count| count.to_str().ok())
        .and_then(|count| count.parse().ok())
        .expect("a total count");
    let body: Value = resp.json().await.expect("Failed to parse the list");
    let uuids = body
        .as_array()
        .expect("a list")
        .iter()
        .map(|changepoint| changepoint["uuid"].as_str().expect("a UUID").to_owned())
        .collect();
    (uuids, total)
}

// The list holds the active changepoints by default, newest first, and each status a user sees
// by its filter, and never a candidate. It filters by branch, testbed, benchmark, and measure,
// leaves out an archived testbed unless asked, sorts by creation, and pages.
#[tokio::test]
async fn changepoints_list_by_status_and_dimension() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "list").await;
    let main = f.line(&server, MAIN);
    let lines = [
        MAIN,
        Line {
            benchmark: "bench-two",
            ..MAIN
        },
        Line {
            branch: "other-branch",
            ..MAIN
        },
        Line {
            testbed: "other-testbed",
            ..MAIN
        },
        Line {
            measure: "throughput",
            ..MAIN
        },
    ]
    .map(|line| f.line(&server, line));
    // Created in one order and modified in the reverse, so the two sorts differ.
    let active = lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let i = i64::try_from(i).expect("an index");
            insert(&server, line, ChangepointState::Active, STEP, i, 100 - i)
        })
        .map(|uuid| uuid.to_string())
        .collect::<Vec<_>>();
    let [dismissed, silenced, withdrawn] = [
        ChangepointState::Dismissed,
        ChangepointState::Silenced,
        ChangepointState::Withdrawn,
    ]
    .map(|state| insert(&server, &main, state, STEP, 10, 10).to_string());
    insert(&server, &main, ChangepointState::Candidate, STEP, 200, 200);

    // An archived testbed is left out unless asked for, as it is in the alerts list.
    let (status, body) = f
        .send(
            &server,
            Method::PATCH,
            "testbeds/other-testbed",
            Some(&json!({ "archived": true })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let picked = |indexes: &[usize]| {
        indexes
            .iter()
            .map(|&i| active[i].clone())
            .collect::<Vec<_>>()
    };
    for (query, expected, total) in [
        ("", picked(&[0, 1, 2, 4]), 4),
        ("?status=active", picked(&[0, 1, 2, 4]), 4),
        ("?direction=asc", picked(&[4, 2, 1, 0]), 4),
        ("?sort=created", picked(&[0, 1, 2, 4]), 4),
        ("?sort=created&direction=desc", picked(&[4, 2, 1, 0]), 4),
        ("?per_page=2&page=2", picked(&[2, 4]), 4),
        ("?status=dismissed", vec![dismissed.clone()], 1),
        ("?status=silenced", vec![silenced.clone()], 1),
        ("?status=withdrawn", vec![withdrawn.clone()], 1),
        ("?benchmark=bench-two", picked(&[1]), 1),
        ("?branch=other-branch", picked(&[2]), 1),
        ("?measure=throughput", picked(&[4]), 1),
        ("?testbed=other-testbed", Vec::new(), 0),
        ("?archived=true", picked(&[3]), 1),
        ("?archived=true&testbed=other-testbed", picked(&[3]), 1),
        (
            "?branch=main-branch&status=dismissed",
            vec![dismissed.clone()],
            1,
        ),
    ] {
        assert_eq!(list(&server, &f, query).await, (expected, total), "{query}");
    }

    let (status, body) = f
        .send(&server, Method::GET, "changepoints?status=candidate", None)
        .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "a candidate is no status a user sees: {body}"
    );
}

// A changepoint names its line, its version, the threshold and the changepoint model it was
// judged against, everything it measured, and the change in percent. A candidate is not found.
#[tokio::test]
#[expect(clippy::too_many_lines, reason = "one assertion per field of the view")]
async fn changepoints_view_names_the_line_and_what_it_measured() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "view").await;
    let main = f.line(&server, MAIN);
    let active = insert(&server, &main, ChangepointState::Active, STEP, 1, 2);
    let flat = insert(
        &server,
        &main,
        ChangepointState::Dismissed,
        Measured {
            mean_before: 0.0,
            mean_after: 5.0,
            tstatistic: None,
        },
        1,
        2,
    );
    let candidate = insert(&server, &main, ChangepointState::Candidate, STEP, 1, 2);

    let (status, threshold) = f
        .send(
            &server,
            Method::GET,
            "thresholds?branch=main-branch&testbed=main-testbed&measure=latency",
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{threshold}");
    let threshold = &threshold[0];

    let (status, changepoint) = f
        .send(
            &server,
            Method::GET,
            &format!("changepoints/{active}"),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{changepoint}");
    assert_eq!(changepoint["uuid"], active.to_string());
    assert_eq!(changepoint["threshold"]["uuid"], threshold["uuid"]);
    assert_eq!(
        changepoint["threshold"]["changepoint_model"]["uuid"],
        threshold["changepoint_model"]["uuid"],
        "the changepoint model it was judged against"
    );
    assert_eq!(changepoint["threshold"]["branch"]["slug"], "main-branch");
    assert_eq!(
        changepoint["threshold"]["branch"]["head"]["uuid"],
        threshold["branch"]["head"]["uuid"]
    );
    let version: i32 = schema::version::table
        .filter(schema::version::id.eq(main.version_id.expect("a version")))
        .select(schema::version::number)
        .first(&mut server.db_conn())
        .expect("Failed to get the version");
    assert_eq!(
        changepoint["threshold"]["branch"]["head"]["version"]["number"],
        version
    );
    assert_eq!(changepoint["threshold"]["testbed"]["slug"], "main-testbed");
    assert_eq!(changepoint["threshold"]["measure"]["slug"], "latency");
    assert_eq!(changepoint["benchmark"]["slug"], "bench-one");
    assert_eq!(changepoint["variant"]["parameters"], json!({}));
    for (field, value) in [
        ("metric", json!("value")),
        ("mean_before", json!(100.0)),
        ("mean_after", json!(110.0)),
        ("stddev_before", json!(2.0)),
        ("stddev_after", json!(2.5)),
        ("pvalue", json!(0.000_1)),
        ("tstatistic", json!(-21.5)),
        ("degrees_of_freedom", json!(98)),
        ("status", json!("active")),
        ("created", json!(seconds(1))),
        ("modified", json!(seconds(2))),
    ] {
        assert_eq!(changepoint[field], value, "{field}");
    }
    let typed: JsonChangepoint = serde_json::from_value(changepoint).expect("a changepoint");
    let percent_change = typed.percent_change.expect("a percent change");
    assert!((percent_change - 10.0).abs() < 1e-9, "{percent_change}");

    let (status, changepoint) = f
        .send(&server, Method::GET, &format!("changepoints/{flat}"), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{changepoint}");
    assert_eq!(
        changepoint["percent_change"],
        json!(0.0),
        "a mean before of 0 gives 0"
    );
    assert_eq!(
        changepoint.get("tstatistic"),
        None,
        "a t statistic stored as none is absent"
    );
    let typed: JsonChangepoint =
        serde_json::from_value(changepoint).expect("a changepoint with no t statistic");
    assert_eq!(typed.tstatistic, None);

    let (status, body) = f
        .send(
            &server,
            Method::GET,
            &format!("changepoints/{candidate}"),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
}

// The view of a changepoint serves the scans that found it, oldest first, each with what it measured
// and its change in percent, and so does a status change. The list serves none.
#[tokio::test]
async fn changepoints_view_serves_its_scans_and_the_list_does_not() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "scans").await;
    let main = f.line(&server, MAIN);
    let active = insert(&server, &main, ChangepointState::Active, STEP, 1, 2);
    // Written out of order, so the view orders them. Means whose change in percent is exact.
    insert_scan(&server, active, 150.0, 2);
    insert_scan(&server, active, 125.0, 1);

    let scans = json!([
        {
            "mean_before": 100.0,
            "mean_after": 125.0,
            "stddev_before": 2.0,
            "stddev_after": 2.5,
            "pvalue": 0.000_1,
            "degrees_of_freedom": 98,
            "percent_change": 25.0,
            "created": seconds(1),
        },
        {
            "mean_before": 100.0,
            "mean_after": 150.0,
            "stddev_before": 2.0,
            "stddev_after": 2.5,
            "pvalue": 0.000_1,
            "degrees_of_freedom": 98,
            "percent_change": 50.0,
            "created": seconds(2),
        },
    ]);
    let (status, changepoint) = f
        .send(
            &server,
            Method::GET,
            &format!("changepoints/{active}"),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{changepoint}");
    assert_eq!(changepoint["scans"], scans, "{changepoint}");
    let typed: JsonChangepoint = serde_json::from_value(changepoint).expect("a changepoint");
    assert_eq!(typed.scans.map(|scans| scans.len()), Some(2));

    let (status, changepoint) = f
        .send(
            &server,
            Method::PATCH,
            &format!("changepoints/{active}"),
            Some(&json!({ "status": "dismissed" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{changepoint}");
    assert_eq!(changepoint["scans"], scans, "{changepoint}");

    let (status, listed) = f
        .send(&server, Method::GET, "changepoints?status=dismissed", None)
        .await;
    assert_eq!(status, StatusCode::OK, "{listed}");
    assert_eq!(listed[0]["uuid"], active.to_string(), "{listed}");
    assert_eq!(listed[0].get("scans"), None, "{listed}");
}

// A threshold update that empties or replaces the changepoint slot deletes the threshold's
// candidates, which no user saw, so they hold no delete back: a PUT that removes the changepoint
// model, after which the threshold deletes, a PUT that replaces it, and a report's reset. Its other
// changepoints stay.
#[tokio::test]
#[expect(clippy::too_many_lines, reason = "one case per way a slot is written")]
async fn a_threshold_update_of_its_changepoint_slot_deletes_its_candidates() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "orphans").await;
    let changepoint_model = json!({
        "test": "changepoint",
        "max_sample_size": 50,
        "upper_boundary": 0.05,
        "significance": 0.001,
    });

    // No report is on the size measure, so only its candidate could hold its threshold.
    let mut size = f.line(
        &server,
        Line {
            measure: "size",
            ..MAIN
        },
    );
    size.version_id = f.line(&server, MAIN).version_id;
    let size_threshold = threshold_uuid(&server, &f, "size").await;
    let (status, body) = f
        .send(
            &server,
            Method::PUT,
            &format!("thresholds/{size_threshold}"),
            Some(&json!({ "test": null })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "only a changepoint model: {body}");
    let orphan = insert(&server, &size, ChangepointState::Candidate, STEP, 1, 2);
    let (status, body) = f
        .send(
            &server,
            Method::PUT,
            &format!("thresholds/{size_threshold}"),
            Some(&json!({ "test": null, "changepoint_model": null })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(stored(&server, orphan), 0, "the removal took the candidate");
    let (status, body) = f
        .send(
            &server,
            Method::DELETE,
            &format!("thresholds/{size_threshold}"),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");

    let main = f.line(&server, MAIN);
    let replaced = insert(&server, &main, ChangepointState::Candidate, STEP, 1, 2);
    let kept = insert(&server, &main, ChangepointState::Active, STEP, 1, 2);
    let main_threshold = threshold_uuid(&server, &f, "latency").await;
    let (status, body) = f
        .send(
            &server,
            Method::PUT,
            &format!("thresholds/{main_threshold}"),
            Some(&json!({
                "test": "t_test",
                "upper_boundary": 0.99,
                "changepoint_model": merge(&changepoint_model, json!({ "max_sample_size": 25 })),
            })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        stored(&server, replaced),
        0,
        "the replacement took the candidate"
    );
    assert_eq!(stored(&server, kept), 1, "an active changepoint stays");

    let throughput = f.line(
        &server,
        Line {
            measure: "throughput",
            ..MAIN
        },
    );
    let reset = insert(
        &server,
        &throughput,
        ChangepointState::Candidate,
        STEP,
        1,
        2,
    );
    let report = json!({
        "branch": "main-branch",
        "testbed": "main-testbed",
        "start_time": "2024-01-04T00:00:00Z",
        "end_time": "2024-01-04T00:01:00Z",
        "results": [json!({ "bench-one": { "latency": { "value": 1.0 } } }).to_string()],
        "thresholds": { "models": { "latency": { "test": "t_test", "upper_boundary": 0.99 } }, "reset": true },
    });
    let (status, body) = f
        .send(&server, Method::POST, "reports", Some(&report))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(stored(&server, reset), 0, "the reset took the candidate");
    assert_eq!(
        stored(&server, kept),
        1,
        "the reset keeps an active changepoint"
    );
    assert_eq!(broken_keys(&server), Vec::<String>::new());
}

// A report that names a new changepoint model for a threshold deletes the threshold's candidates,
// on a line the report does not hold as well, and keeps its other changepoints.
#[tokio::test]
async fn a_report_that_replaces_the_changepoint_model_deletes_its_candidates() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "reportslot").await;
    // The report holds no metric on this line, so only the slot replacement could take its
    // candidate.
    let line = f.line(
        &server,
        Line {
            benchmark: "bench-two",
            ..MAIN
        },
    );
    let candidate = insert(&server, &line, ChangepointState::Candidate, STEP, 1, 2);
    let kept = insert(&server, &line, ChangepointState::Active, STEP, 1, 2);

    let measures = json!({ "latency": { "value": 1.0 } });
    let report = json!({
        "branch": "main-branch",
        "testbed": "main-testbed",
        "start_time": "2024-01-04T00:00:00Z",
        "end_time": "2024-01-04T00:01:00Z",
        "results": [json!({ "bench-one": [{ "parameters": {}, "measures": measures }] }).to_string()],
        "bmf_version": 1,
        "thresholds": { "models": [{
            "measure": "latency",
            "metric": "value",
            "model": {
                "test": "changepoint",
                "max_sample_size": 25,
                "upper_boundary": 0.05,
                "significance": 0.001,
            },
        }] },
    });
    let (status, body) = f
        .send(&server, Method::POST, "reports", Some(&report))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_ne!(
        f.line(&server, MAIN).model_id,
        line.model_id,
        "the report replaced the changepoint model"
    );
    assert_eq!(
        stored(&server, candidate),
        0,
        "the replacement took the candidate"
    );
    assert_eq!(stored(&server, kept), 1, "an active changepoint stays");
    assert_eq!(broken_keys(&server), Vec::<String>::new());
}

// A start point clone that replaces a threshold's changepoint model deletes the threshold's
// candidates and keeps its other changepoints. A reset that names the idempotency key of the
// branch's head reuses that head, so only the clone could take them.
#[tokio::test]
async fn a_start_point_clone_that_replaces_the_changepoint_model_deletes_its_candidates() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "cloneslot").await;
    let (status, body) = f
        .send(
            &server,
            Method::POST,
            "branches",
            Some(&json!({ "name": "feature-branch" })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let reset = json!({
        "start_point": {
            "branch": "main-branch",
            "clone_thresholds": true,
            "reset": true,
            "idempotency_key": HeadIdempotencyKey::new(),
        }
    });
    let (status, body) = f
        .send(
            &server,
            Method::PATCH,
            "branches/feature-branch",
            Some(&reset),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let feature = Line {
        branch: "feature-branch",
        ..MAIN
    };
    let mut line = f.line(&server, feature);
    line.version_id = f.line(&server, MAIN).version_id;
    let candidate = insert(&server, &line, ChangepointState::Candidate, STEP, 1, 2);
    let kept = insert(&server, &line, ChangepointState::Active, STEP, 1, 2);

    let start_point = threshold_uuid(&server, &f, "latency").await;
    let (status, body) = f
        .send(
            &server,
            Method::PUT,
            &format!("thresholds/{start_point}"),
            Some(&json!({
                "test": "t_test",
                "upper_boundary": 0.99,
                "changepoint_model": {
                    "test": "changepoint",
                    "max_sample_size": 25,
                    "upper_boundary": 0.05,
                    "significance": 0.001,
                },
            })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = f
        .send(
            &server,
            Method::PATCH,
            "branches/feature-branch",
            Some(&reset),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let cloned = f.line(&server, feature);
    assert_eq!(cloned.head_id, line.head_id, "the reset reused its head");
    assert_ne!(
        cloned.model_id, line.model_id,
        "the clone replaced the changepoint model"
    );
    assert_eq!(
        stored(&server, candidate),
        0,
        "the clone took the candidate"
    );
    assert_eq!(stored(&server, kept), 1, "an active changepoint stays");
    assert_eq!(broken_keys(&server), Vec::<String>::new());
}

/// The UUID of the threshold on the main branch and testbed for the measure.
async fn threshold_uuid(server: &TestServer, f: &Fixture, measure: &str) -> String {
    let (status, thresholds) = f
        .send(
            server,
            Method::GET,
            &format!("thresholds?branch=main-branch&testbed=main-testbed&measure={measure}"),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{thresholds}");
    thresholds
        .get(0)
        .and_then(|threshold| threshold.get("uuid"))
        .and_then(Value::as_str)
        .expect("a UUID")
        .to_owned()
}

fn merge(model: &Value, fields: Value) -> Value {
    let mut model = model.clone();
    if let (Some(model), Value::Object(fields)) = (model.as_object_mut(), fields) {
        model.extend(fields);
    }
    model
}

// A changepoint names the changepoint model it was judged against, even after a PUT replaces the
// model in the threshold's slot.
#[tokio::test]
async fn changepoints_keep_the_model_they_were_judged_against() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "pinned").await;
    let main = f.line(&server, MAIN);
    let id = insert(&server, &main, ChangepointState::Active, STEP, 1, 2);
    let path = format!("changepoints/{id}");
    let (status, before) = f.send(&server, Method::GET, &path, None).await;
    assert_eq!(status, StatusCode::OK, "{before}");
    let judged = before["threshold"]["changepoint_model"]["uuid"].clone();
    assert!(judged.is_string(), "{before}");

    let replace = json!({
        "test": "t_test",
        "upper_boundary": 0.99,
        "changepoint_model": {
            "test": "changepoint",
            "max_sample_size": 40,
            "upper_boundary": 0.05,
            "significance": 0.01,
        },
    });
    let threshold = before["threshold"]["uuid"].as_str().expect("a UUID");
    let (status, replaced) = f
        .send(
            &server,
            Method::PUT,
            &format!("thresholds/{threshold}"),
            Some(&replace),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{replaced}");
    assert_ne!(
        replaced["changepoint_model"]["uuid"], judged,
        "the PUT replaces the changepoint model"
    );

    let (status, after) = f.send(&server, Method::GET, &path, None).await;
    assert_eq!(status, StatusCode::OK, "{after}");
    assert_eq!(
        after["threshold"]["changepoint_model"]["uuid"], judged,
        "the view names the model the changepoint was judged against"
    );
}

// A changepoint names the spec its line was read under, even after its testbed moves to another
// spec, in the view and in the list.
#[tokio::test]
async fn changepoints_keep_the_spec_their_line_was_read_under() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "spec").await;
    let [(spec_a, uuid_a), (spec_b, uuid_b)] =
        ["spec-a", "spec-b"].map(|slug| create_spec(&server, slug));
    let move_testbed = |spec_id: i32| {
        diesel::update(schema::testbed::table.filter(schema::testbed::slug.eq("main-testbed")))
            .set(schema::testbed::spec_id.eq(spec_id))
            .execute(&mut server.db_conn())
            .expect("Failed to move the testbed");
    };
    move_testbed(spec_a);
    let main = f.line(&server, MAIN);
    assert_eq!(main.spec_id, Some(spec_a));
    let id = insert(&server, &main, ChangepointState::Active, STEP, 1, 2);
    move_testbed(spec_b);

    let (status, testbed) = f
        .send(&server, Method::GET, "testbeds/main-testbed", None)
        .await;
    assert_eq!(status, StatusCode::OK, "{testbed}");
    assert_eq!(
        testbed["spec"]["uuid"],
        uuid_b.to_string(),
        "the testbed moved"
    );

    let (status, changepoint) = f
        .send(&server, Method::GET, &format!("changepoints/{id}"), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{changepoint}");
    assert_eq!(
        changepoint["threshold"]["testbed"]["spec"]["uuid"],
        uuid_a.to_string(),
        "the view names the spec the line was read under"
    );

    let (status, changepoints) = f.send(&server, Method::GET, "changepoints", None).await;
    assert_eq!(status, StatusCode::OK, "{changepoints}");
    assert_eq!(changepoints[0]["uuid"], id.to_string());
    assert_eq!(
        changepoints[0]["threshold"]["testbed"]["spec"]["uuid"],
        uuid_a.to_string(),
        "the list names the spec the line was read under"
    );
}

/// Insert a spec with this slug.
fn create_spec(server: &TestServer, slug: &str) -> (i32, SpecUuid) {
    let uuid = SpecUuid::new();
    let mut conn = server.db_conn();
    diesel::insert_into(schema::spec::table)
        .values((
            schema::spec::uuid.eq(uuid),
            schema::spec::name.eq(slug),
            schema::spec::slug.eq(slug),
            schema::spec::os.eq("linux"),
            schema::spec::architecture.eq("x86_64"),
            schema::spec::cpu.eq(8),
            schema::spec::memory.eq(0x0004_0000_0000i64),
            schema::spec::disk.eq(0x0019_0000_0000i64),
            schema::spec::network.eq(false),
            schema::spec::created.eq(seconds(0)),
            schema::spec::modified.eq(seconds(0)),
        ))
        .execute(&mut conn)
        .expect("Failed to insert the spec");
    let id = schema::spec::table
        .filter(schema::spec::uuid.eq(uuid))
        .select(schema::spec::id)
        .first(&mut conn)
        .expect("Failed to get the spec");
    (id, uuid)
}

// A mean before so small that the change is not finite gives no `percent_change`, so a typed
// client still reads the list and the view.
#[tokio::test]
async fn changepoints_leave_out_a_percent_change_that_is_not_finite() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "percent").await;
    let main = f.line(&server, MAIN);
    let id = insert(
        &server,
        &main,
        ChangepointState::Active,
        Measured {
            mean_before: 1e-310,
            mean_after: 1.0,
            tstatistic: Some(-21.5),
        },
        1,
        2,
    );

    let (status, listed) = f.send(&server, Method::GET, "changepoints", None).await;
    assert_eq!(status, StatusCode::OK, "{listed}");
    assert_eq!(listed[0]["uuid"], id.to_string(), "{listed}");
    assert_eq!(listed[0].get("percent_change"), None, "{listed}");
    let typed: JsonChangepoints =
        serde_json::from_value(listed).expect("a typed client reads the list");
    assert_eq!(typed.0.len(), 1);

    let (status, viewed) = f
        .send(&server, Method::GET, &format!("changepoints/{id}"), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{viewed}");
    assert_eq!(viewed.get("percent_change"), None, "{viewed}");
    assert_eq!(viewed["mean_before"], json!(1e-310));
    let _typed: JsonChangepoint =
        serde_json::from_value(viewed).expect("a typed client reads the view");
}

// Deleting a benchmark or a variant that no report holds takes its changepoints, and only its own.
#[tokio::test]
async fn deleting_a_benchmark_or_a_variant_takes_its_changepoints() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "cascade").await;
    let main = f.line(&server, MAIN);
    let kept = insert(&server, &main, ChangepointState::Active, STEP, 1, 2);

    let (status, body) = f
        .send(
            &server,
            Method::POST,
            "benchmarks",
            Some(&json!({ "name": "unreported" })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let benchmark_id = schema::benchmark::table
        .filter(schema::benchmark::project_id.eq(main.project_id))
        .filter(schema::benchmark::slug.eq("unreported"))
        .select(schema::benchmark::id)
        .first::<i32>(&mut server.db_conn())
        .expect("Failed to get the benchmark");
    let on_benchmark = insert(
        &server,
        &LineIds {
            benchmark_id,
            variant_id: get_empty_variant(&mut server.db_conn(), benchmark_id),
            ..main
        },
        ChangepointState::Active,
        STEP,
        1,
        2,
    );

    let (status, variant) = f
        .send(
            &server,
            Method::POST,
            "benchmarks/bench-one/variants",
            Some(&json!({ "parameters": { "size": 1 } })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{variant}");
    let variant = variant["uuid"].as_str().expect("a UUID").to_owned();
    let variant_id = schema::variant::table
        .filter(schema::variant::uuid.eq(&variant))
        .select(schema::variant::id)
        .first::<i32>(&mut server.db_conn())
        .expect("Failed to get the variant");
    let on_variant = insert(
        &server,
        &LineIds { variant_id, ..main },
        ChangepointState::Dismissed,
        STEP,
        1,
        2,
    );

    let (status, body) = f
        .send(&server, Method::DELETE, "benchmarks/unreported", None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    assert_eq!(stored(&server, on_benchmark), 0, "the benchmark took it");
    assert_eq!(stored(&server, on_variant), 1);

    let (status, body) = f
        .send(
            &server,
            Method::DELETE,
            &format!("benchmarks/bench-one/variants/{variant}"),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    assert_eq!(stored(&server, on_variant), 0, "the variant took it");
    assert_eq!(stored(&server, kept), 1, "the other changepoint stays");
    assert_eq!(broken_keys(&server), Vec::<String>::new());
}

/// The number of changepoint rows with this UUID, whatever their status.
fn stored(server: &TestServer, uuid: ChangepointUuid) -> i64 {
    schema::changepoint::table
        .filter(schema::changepoint::uuid.eq(uuid))
        .count()
        .get_result(&mut server.db_conn())
        .expect("Failed to count the changepoint")
}

// A user moves a changepoint between active and dismissed, both ways. A silenced or withdrawn
// changepoint refuses with a 400 that names its status and keeps it, and a candidate is not found.
#[tokio::test]
async fn changepoints_change_between_active_and_dismissed() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "patch").await;
    let main = f.line(&server, MAIN);
    let id = insert(&server, &main, ChangepointState::Active, STEP, 1, 2);
    let path = format!("changepoints/{id}");

    for status in ["dismissed", "active", "dismissed", "active"] {
        let (code, body) = f
            .send(
                &server,
                Method::PATCH,
                &path,
                Some(&json!({ "status": status })),
            )
            .await;
        assert_eq!(code, StatusCode::OK, "{status}: {body}");
        assert_eq!(body["status"], status);
        assert_ne!(body["modified"], json!(seconds(2)), "{status}");
        let (_, viewed) = f.send(&server, Method::GET, &path, None).await;
        assert_eq!(viewed["status"], status);
    }

    for (state, name) in [
        (ChangepointState::Silenced, "silenced"),
        (ChangepointState::Withdrawn, "withdrawn"),
    ] {
        let id = insert(&server, &main, state, STEP, 1, 2);
        let path = format!("changepoints/{id}");
        for status in ["active", "dismissed"] {
            let (code, body) = f
                .send(
                    &server,
                    Method::PATCH,
                    &path,
                    Some(&json!({ "status": status })),
                )
                .await;
            assert_eq!(code, StatusCode::BAD_REQUEST, "{name} to {status}: {body}");
            assert!(body.to_string().contains(&format!("is {name}")), "{body}");
        }
        let (_, viewed) = f.send(&server, Method::GET, &path, None).await;
        assert_eq!(viewed["status"], name);
        assert_eq!(viewed["modified"], json!(seconds(2)), "{name}");
    }

    let candidate = insert(&server, &main, ChangepointState::Candidate, STEP, 1, 2);
    let (code, body) = f
        .send(
            &server,
            Method::PATCH,
            &format!("changepoints/{candidate}"),
            Some(&json!({ "status": "dismissed" })),
        )
        .await;
    assert_eq!(code, StatusCode::NOT_FOUND, "{body}");
    let (code, body) = f
        .send(
            &server,
            Method::PATCH,
            &path,
            Some(&json!({ "status": "silenced" })),
        )
        .await;
    assert_eq!(
        code,
        StatusCode::BAD_REQUEST,
        "a user cannot silence: {body}"
    );
}

// A viewer may read the changepoints of a project and not change them; a developer and a project
// key may change them; a user with no role may read a public project's and change none.
#[tokio::test]
async fn changepoints_take_the_alerts_permissions() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "auth").await;
    let main = f.line(&server, MAIN);
    let id = insert(&server, &main, ChangepointState::Active, STEP, 1, 2);
    let path = f.path(&format!("changepoints/{id}"));
    let viewer = server
        .signup("Test User", "changepointsviewer@example.com")
        .await;
    let developer = server
        .signup("Test User", "changepointsdeveloper@example.com")
        .await;
    let stranger = server
        .signup("Test User", "changepointsstranger@example.com")
        .await;
    grant_project_role(&server, &viewer, &f.slug, Role::Viewer);
    grant_project_role(&server, &developer, &f.slug, Role::Developer);
    let (status, key) = f
        .send(
            &server,
            Method::POST,
            "keys",
            Some(&json!({ "name": "changepoint-key" })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{key}");
    let key: JsonProjectKeyCreated = serde_json::from_value(key).expect("a project key");

    let dismiss = json!({ "status": "dismissed" });
    for (token, read, change) in [
        (viewer.token.as_str(), StatusCode::OK, StatusCode::FORBIDDEN),
        (
            stranger.token.as_str(),
            StatusCode::OK,
            StatusCode::FORBIDDEN,
        ),
        (developer.token.as_str(), StatusCode::OK, StatusCode::OK),
        (key.key.as_ref(), StatusCode::OK, StatusCode::OK),
    ] {
        let (status, body) = send(&server, token, Method::GET, &path, None).await;
        assert_eq!(status, read, "{body}");
        let (status, body) = send(&server, token, Method::GET, &f.path("changepoints"), None).await;
        assert_eq!(status, read, "{body}");
        let (status, body) = send(&server, token, Method::PATCH, &path, Some(&dismiss)).await;
        assert_eq!(status, change, "{body}");
    }
}

// Resetting a branch through the API silences the active changepoints of its old head and deletes
// its candidates, and a dismissed changepoint keeps its status and `modified`.
#[tokio::test]
async fn a_branch_reset_silences_only_active_changepoints() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "reset").await;
    let main = f.line(&server, MAIN);
    let active = insert(&server, &main, ChangepointState::Active, STEP, 1, 2);
    let dismissed = insert(&server, &main, ChangepointState::Dismissed, STEP, 1, 2);
    let candidate = insert(&server, &main, ChangepointState::Candidate, STEP, 1, 2);
    let old_head: String = schema::head::table
        .filter(schema::head::id.eq(main.head_id))
        .select(schema::head::uuid)
        .first(&mut server.db_conn())
        .expect("Failed to get the old head");
    let version: i32 = schema::version::table
        .filter(schema::version::id.eq(main.version_id.expect("a version")))
        .select(schema::version::number)
        .first(&mut server.db_conn())
        .expect("Failed to get the version");

    let reset = json!({ "start_point": { "reset": true } });
    let (status, body) = f
        .send(&server, Method::PATCH, "branches/main-branch", Some(&reset))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_ne!(body["head"]["uuid"], old_head, "the reset makes a new head");

    let (status, silenced) = f
        .send(
            &server,
            Method::GET,
            &format!("changepoints/{active}"),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{silenced}");
    assert_eq!(silenced["status"], "silenced");
    assert_ne!(silenced["modified"], json!(seconds(2)), "silenced now");
    assert_eq!(
        silenced["threshold"]["branch"]["head"]["uuid"], old_head,
        "a silenced changepoint names the head it was found on"
    );
    assert_eq!(
        silenced["threshold"]["branch"]["head"]["version"]["number"], version,
        "and the version it was recorded at"
    );
    let (_, kept) = f
        .send(
            &server,
            Method::GET,
            &format!("changepoints/{dismissed}"),
            None,
        )
        .await;
    assert_eq!(kept["status"], "dismissed");
    assert_eq!(
        kept["modified"],
        json!(seconds(2)),
        "the dismissal keeps its time"
    );
    let candidates = schema::changepoint::table
        .filter(schema::changepoint::uuid.eq(candidate))
        .count()
        .get_result::<i64>(&mut server.db_conn())
        .expect("Failed to count the candidate");
    assert_eq!(candidates, 0, "the candidate is deleted");
    assert_eq!(
        list(&server, &f, "?status=silenced").await,
        (vec![active.to_string()], 1)
    );
}

// A project lists, views, and changes only its own changepoints, even by another's UUID.
#[tokio::test]
async fn changepoints_belong_to_their_own_project() {
    let server = TestServer::new().await;
    let a = Fixture::new(&server, "owna").await;
    let b = Fixture::new(&server, "ownb").await;
    let a_id = insert(
        &server,
        &a.line(&server, MAIN),
        ChangepointState::Active,
        STEP,
        1,
        2,
    );
    let b_id = insert(
        &server,
        &b.line(&server, MAIN),
        ChangepointState::Active,
        STEP,
        1,
        2,
    );

    assert_eq!(list(&server, &a, "").await, (vec![a_id.to_string()], 1));
    assert_eq!(list(&server, &b, "").await, (vec![b_id.to_string()], 1));
    let path = format!("changepoints/{b_id}");
    let (status, body) = a.send(&server, Method::GET, &path, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let dismiss = json!({ "status": "dismissed" });
    let (status, body) = a.send(&server, Method::PATCH, &path, Some(&dismiss)).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (_, viewed) = b.send(&server, Method::GET, &path, None).await;
    assert_eq!(
        viewed["status"], "active",
        "the other project's changepoint is unchanged"
    );
}

// Deleting a report takes the changepoints on the version it was the last report of, a branch or
// a threshold that a changepoint holds is refused as one that a boundary holds is, and deleting the
// project takes every changepoint. No delete leaves a changepoint that points nowhere.
#[tokio::test]
async fn deletes_leave_no_dangling_changepoint() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "delete").await;
    let main = f.line(&server, MAIN);
    let held = insert(&server, &main, ChangepointState::Active, STEP, 1, 2);
    // No report is on the size measure, so only the changepoint holds its threshold.
    let size = f.line(
        &server,
        Line {
            measure: "size",
            ..MAIN
        },
    );
    let on_size = insert(&server, &size, ChangepointState::Dismissed, STEP, 1, 2);

    let (status, body) = f
        .send(&server, Method::DELETE, "branches/main-branch", None)
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    let (status, thresholds) = f
        .send(&server, Method::GET, "thresholds?measure=size", None)
        .await;
    assert_eq!(status, StatusCode::OK, "{thresholds}");
    let size_threshold = thresholds[0]["uuid"].as_str().expect("a UUID").to_owned();
    let (status, body) = f
        .send(
            &server,
            Method::DELETE,
            &format!("thresholds/{size_threshold}"),
            None,
        )
        .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "a changepoint holds its threshold: {body}"
    );
    let (status, _) = f
        .send(
            &server,
            Method::GET,
            &format!("changepoints/{on_size}"),
            None,
        )
        .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the refused deletes keep the changepoint"
    );
    assert_eq!(broken_keys(&server), Vec::<String>::new());

    // The report on the other testbed is the only report on its head's newest version.
    let other = f.line(
        &server,
        Line {
            testbed: "other-testbed",
            ..MAIN
        },
    );
    let on_other = insert(&server, &other, ChangepointState::Active, STEP, 1, 2);
    let report: String = schema::report::table
        .filter(schema::report::version_id.eq(other.version_id.expect("a version")))
        .select(schema::report::uuid)
        .first(&mut server.db_conn())
        .expect("Failed to get the report");
    let (status, body) = f
        .send(&server, Method::DELETE, &format!("reports/{report}"), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let (status, _) = f
        .send(
            &server,
            Method::GET,
            &format!("changepoints/{on_other}"),
            None,
        )
        .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "the changepoint went with its version"
    );
    let (status, _) = f
        .send(&server, Method::GET, &format!("changepoints/{held}"), None)
        .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a changepoint on another version stays"
    );
    assert_eq!(broken_keys(&server), Vec::<String>::new());

    let (status, body) = f.send(&server, Method::DELETE, "?hard=true", None).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let left = schema::changepoint::table
        .count()
        .get_result::<i64>(&mut server.db_conn())
        .expect("Failed to count the changepoints");
    assert_eq!(left, 0, "the project took its changepoints");
    assert_eq!(broken_keys(&server), Vec::<String>::new());
}

// A threshold delete deletes the threshold's candidates, which no user saw, with their scans, so a
// threshold that only a candidate holds deletes.
#[tokio::test]
async fn a_threshold_delete_deletes_its_candidates() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "deletecandidate").await;
    // No report is on the size measure, so only the candidate could hold its threshold.
    let mut size = f.line(
        &server,
        Line {
            measure: "size",
            ..MAIN
        },
    );
    size.version_id = f.line(&server, MAIN).version_id;
    let candidate = insert(&server, &size, ChangepointState::Candidate, STEP, 1, 2);
    insert_scan(&server, candidate, 110.0, 1);
    let candidate_id = changepoint_id(&server, candidate);

    let size_threshold = threshold_uuid(&server, &f, "size").await;
    let (status, body) = f
        .send(
            &server,
            Method::DELETE,
            &format!("thresholds/{size_threshold}"),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    assert_eq!(
        stored(&server, candidate),
        0,
        "the delete took the candidate"
    );
    assert_eq!(
        scans_of(&server, candidate_id),
        0,
        "the delete took the candidate's scans"
    );
    assert_eq!(broken_keys(&server), Vec::<String>::new());
}

// A threshold delete that a changepoint in any status but a candidate refuses keeps the
// threshold's candidate and its scans: the candidate delete is undone with the refused delete.
#[tokio::test]
async fn a_threshold_delete_refused_by_a_changepoint_keeps_its_candidates() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "refusecandidate").await;
    // No report is on the size measure, so only its changepoints could hold its threshold.
    let mut size = f.line(
        &server,
        Line {
            measure: "size",
            ..MAIN
        },
    );
    size.version_id = f.line(&server, MAIN).version_id;
    let size_threshold = threshold_uuid(&server, &f, "size").await;

    for state in [
        ChangepointState::Active,
        ChangepointState::Dismissed,
        ChangepointState::Silenced,
        ChangepointState::Withdrawn,
    ] {
        let candidate = insert(&server, &size, ChangepointState::Candidate, STEP, 1, 2);
        insert_scan(&server, candidate, 110.0, 1);
        let candidate_id = changepoint_id(&server, candidate);
        let held = insert(&server, &size, state, STEP, 1, 2);

        let (status, body) = f
            .send(
                &server,
                Method::DELETE,
                &format!("thresholds/{size_threshold}"),
                None,
            )
            .await;
        assert_eq!(status, StatusCode::CONFLICT, "{state:?}: {body}");
        assert_eq!(
            stored(&server, candidate),
            1,
            "{state:?}: the refused delete keeps the candidate"
        );
        assert_eq!(
            scans_of(&server, candidate_id),
            1,
            "{state:?}: the refused delete keeps the candidate's scans"
        );

        diesel::delete(
            schema::changepoint::table.filter(schema::changepoint::uuid.eq_any([candidate, held])),
        )
        .execute(&mut server.db_conn())
        .expect("Failed to delete the changepoints");
    }
    assert_eq!(broken_keys(&server), Vec::<String>::new());
}

fn changepoint_id(server: &TestServer, uuid: ChangepointUuid) -> i32 {
    schema::changepoint::table
        .filter(schema::changepoint::uuid.eq(uuid))
        .select(schema::changepoint::id)
        .first(&mut server.db_conn())
        .expect("Failed to get the changepoint")
}

/// The number of scans of the changepoint with this id.
fn scans_of(server: &TestServer, changepoint_id: i32) -> i64 {
    schema::changepoint_scan::table
        .filter(schema::changepoint_scan::changepoint_id.eq(changepoint_id))
        .count()
        .get_result(&mut server.db_conn())
        .expect("Failed to count the scans")
}

fn broken_keys(server: &TestServer) -> Vec<String> {
    raw_rows(
        &mut server.db_conn(),
        "SELECT json_array(\"table\", rowid, parent, fkid) AS row FROM pragma_foreign_key_check",
    )
}

/// The name and the row count of every table, and every object's schema, but the changepoint's and
/// its scans'.
const KEPT_SCHEMA: &str = "SELECT json_array(type, name, sql) AS row FROM sqlite_master WHERE tbl_name NOT IN ('changepoint', 'changepoint_scan') AND name NOT LIKE 'sqlite_%' ORDER BY type, name";
const TABLES: &str = "SELECT name AS row FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name NOT IN ('changepoint', 'changepoint_scan', '__diesel_schema_migrations') ORDER BY name";
/// The columns of `changepoint`, in order.
const CHANGEPOINT_COLUMNS: &str = "SELECT json_array(name, type, \"notnull\") AS row FROM pragma_table_info('changepoint') ORDER BY cid";
/// Every foreign key of `changepoint`.
const CHANGEPOINT_KEYS: &str = "SELECT json_array(\"from\", \"table\", \"to\", on_delete) AS row FROM pragma_foreign_key_list('changepoint') ORDER BY \"from\"";
/// Every index on `changepoint`, by its columns.
const CHANGEPOINT_INDEXES: &str = "SELECT json_array(i.name, (SELECT group_concat(c.name, ',') FROM pragma_index_info(i.name) AS c)) AS row FROM pragma_index_list('changepoint') AS i ORDER BY i.name";
/// The columns of `changepoint_scan`, in order.
const SCAN_COLUMNS: &str = "SELECT json_array(name, type, \"notnull\") AS row FROM pragma_table_info('changepoint_scan') ORDER BY cid";
/// Every foreign key of `changepoint_scan`.
const SCAN_KEYS: &str = "SELECT json_array(\"from\", \"table\", \"to\", on_delete) AS row FROM pragma_foreign_key_list('changepoint_scan') ORDER BY \"from\"";
/// Every index on `changepoint_scan`, by its columns.
const SCAN_INDEXES: &str = "SELECT json_array(i.name, (SELECT group_concat(c.name, ',') FROM pragma_index_info(i.name) AS c)) AS row FROM pragma_index_list('changepoint_scan') AS i ORDER BY i.name";

// The migration down and up keeps every other table, its schema, and its rows, and up makes the
// changepoint and changepoint scan tables with every column, foreign key, and index.
// The server keeps serving on its own threads while this test's connection holds a lock.
#[tokio::test(flavor = "multi_thread")]
#[expect(
    clippy::too_many_lines,
    reason = "one assertion per column, key, and index"
)]
async fn changepoint_migration_down_and_up_keeps_every_table() {
    let server = TestServer::new().await;
    let f = Fixture::new(&server, "migration").await;
    let main = f.line(&server, MAIN);
    let active = insert(&server, &main, ChangepointState::Active, STEP, 1, 2);
    insert_scan(&server, active, 110.0, 1);

    let mut conn = server.db_conn();
    let before = kept(&mut conn);
    assert!(!before.0.is_empty(), "every other table");

    revert_migration(&mut conn);
    assert_eq!(
        kept(&mut conn),
        before,
        "down keeps every other table and row"
    );
    assert_eq!(
        raw_rows(
            &mut conn,
            "SELECT name AS row FROM sqlite_master WHERE tbl_name IN ('changepoint', 'changepoint_scan')"
        ),
        Vec::<String>::new(),
        "down drops the changepoint and changepoint scan tables and their indexes"
    );

    apply_migration(&mut conn);
    assert_eq!(
        kept(&mut conn),
        before,
        "up keeps every other table and row"
    );
    assert_eq!(
        raw_rows(&mut conn, CHANGEPOINT_COLUMNS),
        [
            r#"["id","INTEGER",1]"#,
            r#"["uuid","TEXT",1]"#,
            r#"["project_id","INTEGER",1]"#,
            r#"["head_id","INTEGER",1]"#,
            r#"["testbed_id","INTEGER",1]"#,
            r#"["spec_id","INTEGER",0]"#,
            r#"["benchmark_id","INTEGER",1]"#,
            r#"["variant_id","INTEGER",1]"#,
            r#"["measure_id","INTEGER",1]"#,
            r#"["metric","TEXT",1]"#,
            r#"["version_id","INTEGER",1]"#,
            r#"["threshold_id","INTEGER",1]"#,
            r#"["model_id","INTEGER",1]"#,
            r#"["mean_before","DOUBLE",1]"#,
            r#"["mean_after","DOUBLE",1]"#,
            r#"["stddev_before","DOUBLE",1]"#,
            r#"["stddev_after","DOUBLE",1]"#,
            r#"["pvalue","DOUBLE",1]"#,
            r#"["tstatistic","DOUBLE",0]"#,
            r#"["degrees_of_freedom","INTEGER",1]"#,
            r#"["status","INTEGER",1]"#,
            r#"["created","BIGINT",1]"#,
            r#"["modified","BIGINT",1]"#,
        ]
    );
    assert_eq!(
        raw_rows(&mut conn, CHANGEPOINT_KEYS),
        [
            r#"["benchmark_id","benchmark","id","CASCADE"]"#,
            r#"["head_id","head","id","NO ACTION"]"#,
            r#"["measure_id","measure","id","NO ACTION"]"#,
            r#"["model_id","model","id","NO ACTION"]"#,
            r#"["project_id","project","id","CASCADE"]"#,
            r#"["spec_id","spec","id","SET NULL"]"#,
            r#"["testbed_id","testbed","id","NO ACTION"]"#,
            r#"["threshold_id","threshold","id","NO ACTION"]"#,
            r#"["variant_id","variant","id","CASCADE"]"#,
            r#"["version_id","version","id","CASCADE"]"#,
        ]
    );
    assert_eq!(
        raw_rows(&mut conn, CHANGEPOINT_INDEXES),
        [
            r#"["index_changepoint_benchmark","benchmark_id"]"#,
            r#"["index_changepoint_line","head_id,testbed_id,spec_id,benchmark_id,variant_id,measure_id,metric"]"#,
            r#"["index_changepoint_measure","measure_id"]"#,
            r#"["index_changepoint_model","model_id"]"#,
            r#"["index_changepoint_project_status_modified","project_id,status,modified"]"#,
            r#"["index_changepoint_spec","spec_id"]"#,
            r#"["index_changepoint_testbed","testbed_id"]"#,
            r#"["index_changepoint_threshold","threshold_id"]"#,
            r#"["index_changepoint_variant","variant_id"]"#,
            r#"["index_changepoint_version","version_id"]"#,
            r#"["sqlite_autoindex_changepoint_1","uuid"]"#,
        ]
    );
    assert_eq!(
        raw_rows(&mut conn, SCAN_COLUMNS),
        [
            r#"["id","INTEGER",1]"#,
            r#"["changepoint_id","INTEGER",1]"#,
            r#"["mean_before","DOUBLE",1]"#,
            r#"["mean_after","DOUBLE",1]"#,
            r#"["stddev_before","DOUBLE",1]"#,
            r#"["stddev_after","DOUBLE",1]"#,
            r#"["pvalue","DOUBLE",1]"#,
            r#"["tstatistic","DOUBLE",0]"#,
            r#"["degrees_of_freedom","INTEGER",1]"#,
            r#"["created","BIGINT",1]"#,
        ]
    );
    assert_eq!(
        raw_rows(&mut conn, SCAN_KEYS),
        [r#"["changepoint_id","changepoint","id","CASCADE"]"#]
    );
    assert_eq!(
        raw_rows(&mut conn, SCAN_INDEXES),
        [r#"["index_changepoint_scan_changepoint_created","changepoint_id,created"]"#]
    );
    assert_eq!(
        raw_rows(
            &mut conn,
            "SELECT CAST((SELECT count(*) FROM changepoint) + (SELECT count(*) FROM changepoint_scan) AS TEXT) AS row"
        ),
        ["0"],
        "up starts the tables empty"
    );
    drop(conn);

    let (status, body) = f.send(&server, Method::GET, "changepoints", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

/// Every other table with its row count, and every other object's schema.
fn kept(conn: &mut DbConnection) -> (Vec<String>, Vec<String>) {
    let counts = raw_rows(conn, TABLES)
        .into_iter()
        .map(|table| {
            let count = raw_rows(
                conn,
                &format!("SELECT CAST(count(*) AS TEXT) AS row FROM \"{table}\""),
            );
            format!("{table}: {count:?}")
        })
        .collect();
    (counts, raw_rows(conn, KEPT_SCHEMA))
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

fn revert_migration(conn: &mut DbConnection) {
    conn.batch_execute("PRAGMA foreign_keys = OFF")
        .expect("Failed to disable foreign keys");
    loop {
        let version = conn
            .revert_last_migration(MIGRATIONS)
            .expect("Failed to revert a migration");
        if version.to_string() == CHANGEPOINT_MIGRATION {
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
