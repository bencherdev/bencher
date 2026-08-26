#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! The `bmf_version` key of a report payload, end to end through ingest.
//!
//! The key states the Bencher Metric Format version the whole payload is written
//! in. An absent key is version 0.
//!
//! The first half of this file is what the key does to the results: at version 1
//! the `json` node tries its v1 leaf first and falls back to v0, and at version 0
//! it tries them the other way around. Those tests are about that being a
//! reordering rather than a filter, so every payload lands on the same leaf either
//! way and the key on its own refuses nothing.
//!
//! The second half is the project gate, which is the part that does refuse. A
//! project names the highest version it accepts, and a payload above that gate is
//! turned away whether it declared the version or merely parsed as it. Every
//! project starts at version 0, so a payload that ingested before the gate existed
//! ingests still, with one deliberate exception: v1 results reaching a v1 leaf with
//! no declared key, either through the `json_v1` adapter named outright or through
//! the fallback the `magic` and `json` nodes do.

use bencher_api_tests::{TestServer, TestUser};
use bencher_json::{BmfVersion, ProjectSlug};
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
///
/// The user is the server's first signup, which makes them its admin, so this
/// fixture is also what moves the project's BMF version gate.
struct Fixture {
    project_slug: ProjectSlug,
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
        project_slug: project.slug,
        user,
    }
}

/// The same fixture with the project's gate raised to BMF version 1.
///
/// Every payload that states version 1, by declaring the key or by parsing as v1,
/// needs this: a project accepts version 0 and nothing above it until an admin
/// says otherwise.
async fn gated_fixture(server: &TestServer, label: &str) -> Fixture {
    let fixture = fixture(server, label).await;
    server.set_bmf_version(&fixture.project_slug, BmfVersion::V1);
    fixture
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
    let fixture = gated_fixture(&server, "v1").await;
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
    let fixture = gated_fixture(&server, "fallback").await;
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
    let fixture = gated_fixture(&server, "leaf").await;
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
    let fixture = gated_fixture(&server, "fold").await;
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
    let fixture = gated_fixture(&server, "foldv1").await;
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

// --- The project gate ---
//
// A project declares the highest BMF payload version it accepts. It defaults to 0
// everywhere, only a server admin can raise it, and it is visible to everyone.

/// GET one project as `user`.
async fn get_project(
    server: &TestServer,
    user: &TestUser,
    project: &ProjectSlug,
) -> serde_json::Value {
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/projects/{project}")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    let status = resp.status();
    assert_eq!(status, StatusCode::OK, "GET project: {status}");
    resp.json().await.expect("Failed to parse the project")
}

/// PATCH one project with whatever body, and return its status and body.
async fn try_patch(
    server: &TestServer,
    user: &TestUser,
    project: &ProjectSlug,
    body: &serde_json::Value,
) -> (StatusCode, String) {
    let resp = server
        .client
        .patch(server.api_url(&format!("/v0/projects/{project}")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(body)
        .send()
        .await
        .expect("Request failed");
    let status = resp.status();
    let body = resp.text().await.expect("Failed to read the response");
    (status, body)
}

/// What an error response said, without the request id the server mints for it.
fn message(body: &str) -> String {
    let error: serde_json::Value = serde_json::from_str(body).expect("Failed to parse the error");
    error
        .get("message")
        .and_then(serde_json::Value::as_str)
        .expect("the error carries a message")
        .to_owned()
}

/// The refusal has to name the payload's version and the project's, so the message
/// says both what was sent and what the project would take.
///
/// The whole phrase is asserted, and against the message rather than the body: the
/// request id the server mints is a uuid, so it carries digits of its own and an
/// assertion against the body would pass on the id alone.
fn assert_gate_refusal(status: StatusCode, body: &str, payload: u8, project: u8) {
    assert_eq!(status, StatusCode::LOCKED, "{body}");
    let message = message(body);
    let named =
        format!("BMF version {payload}, but this project accepts BMF version {project} at most.");
    assert!(
        message.contains(&named),
        "expected the refusal to say {named:?}: {message}"
    );
}

/// A new project accepts BMF version 0 and says so.
#[tokio::test]
async fn project_bmf_version_defaults_to_0() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "default").await;

    let project = get_project(&server, &fixture.user, &fixture.project_slug).await;
    assert_eq!(project["bmf_version"], serde_json::json!(0));

    server.close().await;
}

/// A payload that declares a version above the gate is refused, and the same
/// project still takes a version 0 payload.
#[tokio::test]
async fn report_declared_version_above_the_gate_is_refused() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "declared").await;
    let results = v0_results();

    let (status, body) = try_report(
        &server,
        &fixture,
        Post {
            results: vec![&results],
            bmf_version: Some(serde_json::json!(1)),
            ..Post::default()
        },
    )
    .await;
    assert_gate_refusal(status, &body, 1, 0);

    // The gate is about the version, not about the project: version 0 still ingests.
    report(
        &server,
        &fixture,
        Post {
            results: vec![&results],
            bmf_version: Some(serde_json::json!(0)),
            ..Post::default()
        },
    )
    .await;

    server.close().await;
}

/// Results that parse as v1 are refused even when no version was declared.
///
/// These are the three side doors the declared key does not cover: the `json_v1`
/// leaf named outright, the `json` node falling back to that leaf after its v0 leaf
/// fails, and the default `magic` adapter reaching the same fallback through the
/// node. All three are refused, and refused the same way, because all three are the
/// same statement about the payload.
#[tokio::test]
async fn report_parsed_v1_above_the_gate_is_refused() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "parsed").await;
    let results = v1_results();

    let mut refusals = Vec::new();
    for adapter in [None, Some("json"), Some("json_v1")] {
        let (status, body) = try_report(
            &server,
            &fixture,
            Post {
                results: vec![&results],
                adapter,
                ..Post::default()
            },
        )
        .await;
        assert_gate_refusal(status, &body, 1, 0);
        refusals.push(message(&body));
    }

    // The same refusal, not just the same class. Only the request id differs,
    // since the server mints a fresh one per request, and that is not compared.
    assert_eq!(refusals.len(), 3);
    for refusal in &refusals {
        assert_eq!(refusal, &refusals[0], "every side door is refused alike");
    }

    server.close().await;
}

/// An admin raises the gate, and every payload the gate refused ingests.
#[tokio::test]
async fn admin_raising_the_gate_admits_the_refused_payloads() {
    let server = TestServer::new().await;
    let fixture = fixture(&server, "raise").await;
    let v0 = v0_results();
    let v1 = v1_results();

    let refused = || {
        [
            Post {
                results: vec![&v0],
                bmf_version: Some(serde_json::json!(1)),
                ..Post::default()
            },
            Post {
                results: vec![&v1],
                ..Post::default()
            },
            Post {
                results: vec![&v1],
                adapter: Some("json_v1"),
                ..Post::default()
            },
        ]
    };

    for post in refused() {
        let (status, body) = try_report(&server, &fixture, post).await;
        assert_gate_refusal(status, &body, 1, 0);
    }

    let (status, body) = try_patch(
        &server,
        &fixture.user,
        &fixture.project_slug,
        &serde_json::json!({ "bmf_version": 1 }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let project = get_project(&server, &fixture.user, &fixture.project_slug).await;
    assert_eq!(project["bmf_version"], serde_json::json!(1));

    for post in refused() {
        report(&server, &fixture, post).await;
    }

    server.close().await;
}

/// The gate is a maximum, so a raised project takes every lower version unchanged.
#[tokio::test]
async fn a_raised_gate_still_accepts_version_0_payloads() {
    let server = TestServer::new().await;
    let fixture = gated_fixture(&server, "maximum").await;
    let results = v0_results();

    let absent = report(
        &server,
        &fixture,
        Post {
            results: vec![&results],
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

    server.close().await;
}

/// Nothing ratchets: the gate is a plain setting an admin can put back.
#[tokio::test]
async fn admin_can_lower_the_gate() {
    let server = TestServer::new().await;
    let fixture = gated_fixture(&server, "lower").await;
    let results = v1_results();

    report(
        &server,
        &fixture,
        Post {
            results: vec![&results],
            bmf_version: Some(serde_json::json!(1)),
            ..Post::default()
        },
    )
    .await;

    let (status, body) = try_patch(
        &server,
        &fixture.user,
        &fixture.project_slug,
        &serde_json::json!({ "bmf_version": 0 }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let project = get_project(&server, &fixture.user, &fixture.project_slug).await;
    assert_eq!(project["bmf_version"], serde_json::json!(0));

    let (status, body) = try_report(
        &server,
        &fixture,
        Post {
            results: vec![&results],
            bmf_version: Some(serde_json::json!(1)),
            ..Post::default()
        },
    )
    .await;
    assert_gate_refusal(status, &body, 1, 0);

    server.close().await;
}

/// Only a server admin can move the gate, and the rest of the patch is unaffected.
///
/// The second user owns their own organization and project, so they are allowed to
/// edit it. The only thing standing between them and the field is the admin check.
#[tokio::test]
async fn non_admin_cannot_move_the_gate() {
    let server = TestServer::new().await;
    // The first signup is the server admin, so the second one is not.
    let _admin = fixture(&server, "owner").await;
    let user = server.signup("Other User", "bmfother@example.com").await;
    let org = server.create_org(&user, "Bmf Other Org").await;
    let project = server
        .create_project(&user, &org, "Bmf Other Project")
        .await;

    let (status, body) = try_patch(
        &server,
        &user,
        &project.slug,
        &serde_json::json!({ "bmf_version": 1 }),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert!(body.contains("bmf_version"), "{body}");

    // The same patch without the field is the patch it has always been.
    let (status, body) = try_patch(
        &server,
        &user,
        &project.slug,
        &serde_json::json!({ "name": "Bmf Renamed Project" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let project = get_project(&server, &user, &project.slug).await;
    assert_eq!(project["name"], serde_json::json!("Bmf Renamed Project"));
    assert_eq!(project["bmf_version"], serde_json::json!(0));

    server.close().await;
}
