#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! The `bmf_version` key of a report payload, end to end through ingest.
//!
//! The key states the Bencher Metric Format version the whole payload is written
//! in. An absent key is version 0. In this layer the key does one thing to the
//! results: at version 1 the `json` node tries its v1 leaf first and falls back to
//! v0, and at version 0 it tries them the other way around. Every test here is
//! about that being a reordering rather than a filter, so no payload that ingested
//! before is refused now and every payload lands on the same leaf either way.

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

/// A BMF v1 payload: a benchmark maps to an array of grid points.
fn v1_results() -> String {
    serde_json::json!({
        "bench_a": [{
            "parameters": { "size_mb": 16 },
            "measures": { "latency": { "value": 42.0 } },
        }]
    })
    .to_string()
}

/// The one payload both JSON leaves claim, and so the one payload whose parsed
/// version the declared version moves.
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
///
/// The same payload at version 0 takes today's path, where the v0 leaf fails and
/// the v1 leaf catches it second, and produces the identical report. That is what
/// pins the key as a reordering of the attempts rather than a change of outcome.
#[tokio::test]
async fn report_bmf_version_1_ingests_a_v1_payload_identically_to_version_0() {
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

    assert_eq!(one, zero);

    // The comparison is not vacuous: a different payload is a different report,
    // and the normalization keeps that difference visible.
    let v0 = v0_results();
    let v0 = report(
        &server,
        &fixture,
        Post {
            results: vec![&v0],
            bmf_version: Some(serde_json::json!(0)),
            ..Post::default()
        },
    )
    .await;
    assert_ne!(one, v0);

    server.close().await;
}

/// Version 1 does not refuse a v0 payload: the v0 leaf still catches it, second.
#[tokio::test]
async fn report_bmf_version_1_still_ingests_a_v0_payload() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "fallback").await;
    let results = v0_results();

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

    assert_eq!(one, zero);
    server.close().await;
}

/// An unknown version is rejected, and the rejection names the accepted versions.
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
        let (status, body) = try_report(
            &server,
            &fixture,
            Post {
                results: vec![&results],
                bmf_version: Some(version.clone()),
                ..Post::default()
            },
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{version}: {body}");
        assert!(
            body.contains("0 or 1"),
            "expected the rejection of {version} to name the accepted versions: {body}"
        );
    }

    server.close().await;
}

/// An explicitly named leaf is an exact statement, so `bmf_version` does not move it.
///
/// The discriminating case is the leaf pointed at the other version's shape: if the
/// key could override the named adapter, a v1 payload would ingest through `json_v0`.
/// It does not, and the v0 payload the leaf does claim ingests exactly as it would
/// at version 0.
#[tokio::test]
async fn report_explicit_leaf_wins_over_bmf_version() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "leaf").await;
    let v1 = v1_results();
    let v0 = v0_results();

    let (status, body) = try_report(
        &server,
        &fixture,
        Post {
            results: vec![&v1],
            bmf_version: Some(serde_json::json!(1)),
            adapter: Some("json_v0"),
            ..Post::default()
        },
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "expected json_v0 to refuse a v1 payload at version 1: {body}"
    );

    let one = report(
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
    let zero = report(
        &server,
        &fixture,
        Post {
            results: vec![&v0],
            bmf_version: Some(serde_json::json!(0)),
            adapter: Some("json_v0"),
            ..Post::default()
        },
    )
    .await;
    assert_eq!(one, zero);

    server.close().await;
}

/// Fold lands on the same report at either declared version.
///
/// Fold is all or nothing across the results array and is refused for BMF v1, so
/// a single iteration reading as v1 would drop the whole report from one folded
/// iteration to one iteration per result, which is a different number of metric
/// rows. The empty payload is exactly such an iteration at version 1, so it is the
/// first case here; a pure v0 array is the second, as the control.
///
/// The assertion is between the two versions rather than about the folded value.
/// The mean divides by the length of the array, so an empty iteration dilutes it,
/// and that is existing behavior this layer deliberately leaves where it found it.
#[tokio::test]
async fn report_fold_is_unmoved_by_the_declared_version() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "fold").await;
    let ten = v0_value(10.0);
    let twenty = v0_value(20.0);

    for results in [
        vec![EMPTY, ten.as_str(), twenty.as_str()],
        vec![ten.as_str(), twenty.as_str()],
    ] {
        for fold in ["min", "max", "mean", "median"] {
            let one = report(
                &server,
                &fixture,
                Post {
                    results: results.clone(),
                    bmf_version: Some(serde_json::json!(1)),
                    fold: Some(fold),
                    ..Post::default()
                },
            )
            .await;
            let zero = report(
                &server,
                &fixture,
                Post {
                    results: results.clone(),
                    bmf_version: Some(serde_json::json!(0)),
                    fold: Some(fold),
                    ..Post::default()
                },
            )
            .await;
            assert_eq!(one, zero, "{fold} over {} results", results.len());

            // One folded iteration, not one per result. This is what the metric
            // row count follows, so it is asserted rather than implied.
            assert_eq!(iterations(&one), 1, "{fold} over {} results", results.len());
        }
    }

    server.close().await;
}

/// A payload that actually reported something as v1 still refuses fold.
///
/// The refusal is a warning and an unfolded ingest, never a rejection, because a
/// harness upgrade must not turn a pipeline red. The declared version does not
/// change that either way: refusal keys on what was parsed.
#[tokio::test]
async fn report_fold_is_still_refused_for_a_v1_payload() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "foldv1").await;
    let v1 = v1_results();

    let one = report(
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
    let zero = report(
        &server,
        &fixture,
        Post {
            results: vec![&v1, &v1],
            bmf_version: Some(serde_json::json!(0)),
            fold: Some("mean"),
            ..Post::default()
        },
    )
    .await;

    assert_eq!(one, zero);
    // Unfolded: one iteration per result, which is what refusing the fold means.
    assert_eq!(iterations(&one), 2);

    server.close().await;
}
