#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! The `bmf_version` key of a report payload, end to end through ingest.
//!
//! The key is a contract: the Bencher Metric Format version it declares is what
//! the whole payload is, and an absent key is version 0. The `json` node parses
//! with that version's leaf only, and a payload whose parsed version differs from
//! its declared version is refused with a 400 that names both versions. Fold is
//! refused for every v1 payload, the empty payload included.

use bencher_api_tests::{TestServer, TestUser};
use http::StatusCode;

/// A BMF v0 payload: a benchmark maps straight to its measures.
fn v0_results() -> String {
    serde_json::json!({
        "bench_a": {
            "latency": {
                "value": 42.0,
                "lower_value": 40.0,
                "upper_value": 44.0,
            }
        }
    })
    .to_string()
}

/// A BMF v0 payload reporting one plain value, for the fold comparisons.
fn v0_value(value: f64) -> String {
    serde_json::json!({ "bench_a": { "latency": { "value": value } } }).to_string()
}

/// A BMF v1 payload: a benchmark maps to an array of variants.
fn v1_results() -> String {
    serde_json::json!({
        "bench_a": [{
            "parameters": { "size_mb": 16 },
            "measures": { "latency": { "value": 42.0 } },
        }]
    })
    .to_string()
}

/// The one payload both JSON leaves claim.
const EMPTY: &str = "{}";

/// A signed up user with an organization and a project to report into.
struct Fixture {
    project_slug: String,
    user: TestUser,
}

async fn fixture(server: &TestServer, label: &str) -> Fixture {
    let user = server
        .signup("Test User", &format!("bmf{label}@example.com"))
        .await;
    let org = server.create_org(&user, &format!("Bmf Org {label}")).await;
    let project = server
        .create_project(&user, &org, &format!("Bmf Project {label}"))
        .await;
    Fixture {
        project_slug: project.slug.to_string(),
        user,
    }
}

/// One report request.
///
/// `bmf_version` tells the three cases apart that this file is about: `None` omits
/// the key from the body entirely, `Some(Value::Null)` sends it as `null`, and any
/// other `Some` sends that value. Omitted and `null` both deserialize to no
/// version, but only one of them is what a client that has never heard of the key
/// sends, so the absent case has to actually be absent.
#[derive(Default)]
struct Post<'a> {
    results: Vec<&'a str>,
    bmf_version: Option<serde_json::Value>,
    adapter: Option<&'a str>,
    fold: Option<&'a str>,
}

/// Post one report and return its status and body, whatever they are.
///
/// Every report a comparison uses carries the same `hash`, so they all resolve to
/// the same branch version and the responses differ only in minted identity.
async fn try_report(
    server: &TestServer,
    fixture: &Fixture,
    post: Post<'_>,
) -> (StatusCode, String) {
    let Post {
        results,
        bmf_version,
        adapter,
        fold,
    } = post;
    let settings = (adapter.is_some() || fold.is_some()).then(|| {
        serde_json::json!({
            "adapter": adapter,
            "fold": fold,
        })
    });
    let mut body = serde_json::json!({
        "branch": "main",
        "testbed": "localhost",
        "hash": "bd8a3ef7c86f5cd1c96e2b5b8a0d3a1e6f4c9b27",
        "start_time": "2024-01-01T00:00:00Z",
        "end_time": "2024-01-01T00:01:00Z",
        "results": results,
        "settings": settings,
    });
    if let Some(bmf_version) = bmf_version
        && let Some(object) = body.as_object_mut()
    {
        object.insert("bmf_version".to_owned(), bmf_version);
    }

    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{}/reports", fixture.project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&fixture.user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    let status = resp.status();
    let body = resp.text().await.expect("Failed to read the response");
    (status, body)
}

/// Post one report, require it to be created, and return the normalized response.
async fn report(server: &TestServer, fixture: &Fixture, post: Post<'_>) -> serde_json::Value {
    let (status, body) = try_report(server, fixture, post).await;
    assert_eq!(status, StatusCode::CREATED, "POST report: {body}");
    let report: serde_json::Value =
        serde_json::from_str(&body).expect("Failed to parse the report");
    normalize(&report, &mut Vec::new())
}

/// Everything a report response says, minus the identity the server mints for it.
///
/// Two reports of the same payload are two rows, so their uuids and creation times
/// differ no matter what. Every uuid is replaced by the position it was first seen
/// at, which keeps aliasing intact: a uuid that repeats within a response still
/// repeats after normalization, and a response that names one uuid where the other
/// names two still differs. Everything else, every measured value, every count,
/// every alert, and the echoed adapter, is compared as it came off the wire.
fn normalize(value: &serde_json::Value, ids: &mut Vec<String>) -> serde_json::Value {
    match value {
        serde_json::Value::String(s) => {
            if is_uuid(s) {
                let index = ids.iter().position(|id| id == s).unwrap_or_else(|| {
                    ids.push(s.clone());
                    ids.len() - 1
                });
                serde_json::Value::String(format!("<uuid {index}>"))
            } else {
                value.clone()
            }
        },
        serde_json::Value::Array(array) => {
            serde_json::Value::Array(array.iter().map(|v| normalize(v, ids)).collect())
        },
        serde_json::Value::Object(object) => serde_json::Value::Object(
            object
                .iter()
                .map(|(key, value)| {
                    let value = if matches!(key.as_str(), "created" | "modified") {
                        serde_json::json!("<time>")
                    } else {
                        normalize(value, ids)
                    };
                    (key.clone(), value)
                })
                .collect(),
        ),
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {
            value.clone()
        },
    }
}

/// How many iterations a report response carries, which is what the metric row
/// count follows.
fn iterations(report: &serde_json::Value) -> usize {
    report
        .get("results")
        .and_then(serde_json::Value::as_array)
        .expect("Report results")
        .len()
}

/// How many reports the project holds, so a rejection can be shown to create none.
async fn report_count(server: &TestServer, fixture: &Fixture) -> usize {
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{}/reports", fixture.project_slug)))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&fixture.user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK, "GET reports");
    let reports: serde_json::Value = resp.json().await.expect("Failed to parse the reports");
    reports.as_array().expect("Reports are an array").len()
}

/// Post one report, require a 400, and return the body so the caller can say what
/// the refusal has to name.
async fn refused(server: &TestServer, fixture: &Fixture, post: Post<'_>) -> String {
    let (status, body) = try_report(server, fixture, post).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "POST report: {body}");
    body
}

/// A refusal under the contract names the parsed version and the declared one,
/// and carries no adapter hint since the adapter is right and the key is wrong.
fn assert_names_both_versions(body: &str, parsed: u8, declared: u8) {
    assert!(
        body.contains(&format!("parsed as BMF version {parsed}"))
            && body.contains(&format!("declared version {declared}")),
        "expected the refusal to name both versions: {body}"
    );
    assert!(
        !body.contains("right adapter"),
        "expected no adapter hint on a version refusal: {body}"
    );
}

fn is_uuid(s: &str) -> bool {
    s.len() == 36
        && s.chars().enumerate().all(|(index, c)| match index {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_hexdigit(),
        })
}

/// A payload with no `bmf_version` key at all is version 0.
///
/// An explicit `null` says the same thing, and both say what an explicit 0 says.
#[tokio::test]
async fn report_absent_bmf_version_is_version_0() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "absent").await;
    let results = v0_results();

    let absent = report(
        &server,
        &fixture,
        Post {
            results: vec![&results],
            bmf_version: None,
            ..Post::default()
        },
    )
    .await;
    let null = report(
        &server,
        &fixture,
        Post {
            results: vec![&results],
            bmf_version: Some(serde_json::Value::Null),
            ..Post::default()
        },
    )
    .await;
    let zero = report(
        &server,
        &fixture,
        Post {
            results: vec![&results],
            bmf_version: Some(serde_json::json!(0)),
            ..Post::default()
        },
    )
    .await;

    assert_eq!(absent, zero);
    assert_eq!(null, zero);
    server.close().await;
}

/// A v1 payload ingests at version 1 through the `json` node, without naming a leaf.
#[tokio::test]
async fn report_bmf_version_1_ingests_a_v1_payload() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "v1").await;
    let results = v1_results();

    let one = report(
        &server,
        &fixture,
        Post {
            results: vec![&results],
            bmf_version: Some(serde_json::json!(1)),
            ..Post::default()
        },
    )
    .await;

    // The variant's parameter set is the proof the v1 shape was read: a v0
    // payload has no parameters to report.
    let parameter_set = one
        .pointer("/results/0/0/parameter/set")
        .and_then(serde_json::Value::as_object)
        .expect("Report result parameter set");
    assert!(!parameter_set.is_empty(), "{one}");

    server.close().await;
}

/// A v1 payload declared as version 0, or with no key, is refused by name: there
/// is no undeclared path to v1.
#[tokio::test]
async fn report_bmf_version_0_refuses_a_v1_payload() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "v1atv0").await;
    let results = v1_results();

    for bmf_version in [None, Some(serde_json::json!(0))] {
        let body = refused(
            &server,
            &fixture,
            Post {
                results: vec![&results],
                bmf_version,
                ..Post::default()
            },
        )
        .await;
        assert_names_both_versions(&body, 1, 0);
    }

    server.close().await;
}

/// A v0 payload declared as version 1 is refused by name.
#[tokio::test]
async fn report_bmf_version_1_refuses_a_v0_payload() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "v0atv1").await;
    let results = v0_results();

    let body = refused(
        &server,
        &fixture,
        Post {
            results: vec![&results],
            bmf_version: Some(serde_json::json!(1)),
            ..Post::default()
        },
    )
    .await;
    assert_names_both_versions(&body, 0, 1);

    server.close().await;
}

/// An unknown version is rejected before anything is created, and the rejection
/// names the accepted versions.
///
/// Every rejected shape is here, because two different messages carry the list: an
/// integer outside the range is reported by the validation error and anything that
/// is not an unsigned integer at all is reported by the deserializer.
#[tokio::test]
async fn report_unknown_bmf_version_is_rejected() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "unknown").await;
    let results = v0_results();

    for version in [
        serde_json::json!(2),
        serde_json::json!(255),
        serde_json::json!(256),
        serde_json::json!(-1),
        serde_json::json!("1"),
        serde_json::json!(1.5),
        serde_json::json!(true),
    ] {
        let body = refused(
            &server,
            &fixture,
            Post {
                results: vec![&results],
                bmf_version: Some(version.clone()),
                ..Post::default()
            },
        )
        .await;
        assert!(
            body.contains("0 or 1"),
            "expected the rejection of {version} to name the accepted versions: {body}"
        );
        assert_eq!(
            report_count(&server, &fixture).await,
            0,
            "expected the rejection of {version} to create nothing"
        );
    }

    server.close().await;
}

/// An explicitly named leaf parses its one shape whatever was declared, so it is
/// held to the contract like any other adapter.
#[tokio::test]
async fn report_explicit_leaf_is_held_to_the_contract() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "leaf").await;
    let v1 = v1_results();
    let v0 = v0_results();

    let one = report(
        &server,
        &fixture,
        Post {
            results: vec![&v1],
            bmf_version: Some(serde_json::json!(1)),
            adapter: Some("json_v1"),
            ..Post::default()
        },
    )
    .await;
    assert_eq!(iterations(&one), 1, "json_v1 at version 1 ingests");

    let body = refused(
        &server,
        &fixture,
        Post {
            results: vec![&v1],
            bmf_version: Some(serde_json::json!(0)),
            adapter: Some("json_v1"),
            ..Post::default()
        },
    )
    .await;
    assert_names_both_versions(&body, 1, 0);

    let body = refused(
        &server,
        &fixture,
        Post {
            results: vec![&v0],
            bmf_version: Some(serde_json::json!(1)),
            adapter: Some("json_v0"),
            ..Post::default()
        },
    )
    .await;
    assert_names_both_versions(&body, 0, 1);

    server.close().await;
}

/// At version 0 an empty iteration folds beside the v0 iterations as it always
/// has: one folded iteration, which is what the metric row count follows.
#[tokio::test]
async fn report_fold_folds_v0_with_an_empty_iteration() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "fold").await;
    let ten = v0_value(10.0);
    let twenty = v0_value(20.0);

    for fold in ["min", "max", "mean", "median"] {
        let folded = report(
            &server,
            &fixture,
            Post {
                results: vec![EMPTY, &ten, &twenty],
                bmf_version: Some(serde_json::json!(0)),
                fold: Some(fold),
                ..Post::default()
            },
        )
        .await;
        assert_eq!(iterations(&folded), 1, "{fold}");
    }

    server.close().await;
}

/// A v1 payload refuses fold: the refusal is a warning and an unfolded ingest,
/// never a rejection, because a harness upgrade must not turn a pipeline red.
#[tokio::test]
async fn report_fold_is_refused_for_a_v1_payload() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "foldv1").await;
    let v1 = v1_results();

    let unfolded = report(
        &server,
        &fixture,
        Post {
            results: vec![&v1, &v1],
            bmf_version: Some(serde_json::json!(1)),
            fold: Some("mean"),
            ..Post::default()
        },
    )
    .await;
    // Unfolded: one iteration per result, which is what refusing the fold means.
    assert_eq!(
        iterations(&unfolded),
        2,
        "a refused fold ingests one iteration per result"
    );

    server.close().await;
}

/// An empty payload declared as version 1 is a v1 payload, so it refuses fold too
/// and still ingests: an empty iteration writes no rows either way, so the
/// response cannot tell folded from unfolded, and the adapter's unit test pins it.
#[tokio::test]
async fn report_fold_is_refused_for_an_empty_v1_payload() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "foldempty").await;

    let unfolded = report(
        &server,
        &fixture,
        Post {
            results: vec![EMPTY],
            bmf_version: Some(serde_json::json!(1)),
            fold: Some("mean"),
            ..Post::default()
        },
    )
    .await;
    assert_eq!(
        iterations(&unfolded),
        0,
        "an empty iteration writes no rows"
    );

    server.close().await;
}
