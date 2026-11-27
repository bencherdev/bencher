#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! `GET /v0/projects/{project}/badge`: a project's status badge, `failing` while
//! the project has an active alert the console's Alerts tab counts.
//!
//! Every alert here is raised by reports posted through the API, so the badge
//! reads the alerts the thresholds really raise.

use api_projects::badge::{FAILING_SVG, PASSING_SVG};
use bencher_api_tests::{TestServer, TestUser};
use http::StatusCode;

const PUBLIC_CACHE_CONTROL: &str = "public, max-age=300, s-maxage=300";
const PRIVATE_CACHE_CONTROL: &str = "private, max-age=300";

/// The badge of the project, with the query string and token when given.
async fn badge(
    server: &TestServer,
    slug: &str,
    query: &str,
    token: Option<&str>,
) -> reqwest::Response {
    let request = server
        .client
        .get(server.api_url(&format!("/v0/projects/{slug}/badge{query}")));
    let request = match token {
        Some(token) => request.header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(token),
        ),
        None => request,
    };
    request.send().await.expect("Request failed")
}

/// The body of the signed out badge, which must be served.
async fn badge_body(server: &TestServer, slug: &str, query: &str) -> String {
    let resp = badge(server, slug, query, None).await;
    assert_eq!(resp.status(), StatusCode::OK, "{slug}{query}");
    resp.text().await.expect("Failed to read the badge")
}

fn header(resp: &reqwest::Response, name: http::header::HeaderName) -> Option<&str> {
    resp.headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
}

/// A public project with one active alert on the `latency` measure, raised on the `branch`
/// and the `localhost` testbed, plus a `feature` branch and an `other` testbed with no
/// alerts. Returns the project slug and the alert UUID.
async fn alerting_project(
    server: &TestServer,
    label: &str,
    branch: &str,
) -> (TestUser, String, String) {
    let user = server
        .signup("Test User", &format!("badge{label}@example.com"))
        .await;
    let org = server
        .create_org(&user, &format!("Badge Org {label}"))
        .await;
    let project = server
        .create_project(&user, &org, &format!("Badge Project {label}"))
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

    let mut alerts = Vec::new();
    for (day, value) in [10.0, 10.1, 9.9, 10.0, 10.2, 1_000.0]
        .into_iter()
        .enumerate()
    {
        let results = serde_json::json!({
            "bench": [{ "parameters": { "size_mb": 16 }, "measures": { "latency": { "value": value } } }]
        })
        .to_string();
        let body = serde_json::json!({
            "branch": branch,
            "testbed": "localhost",
            "start_time": format!("2024-01-{:02}T00:00:00Z", day + 1),
            "end_time": format!("2024-01-{:02}T00:01:00Z", day + 1),
            "results": [results],
            "bmf_version": 1,
            "thresholds": {
                "models": [{
                    "measure": "latency",
                    "metric": "value",
                    "model": {
                        "test": "t_test",
                        "min_sample_size": 2,
                        "max_sample_size": 64,
                        "upper_boundary": 0.98,
                    },
                }]
            },
        });
        let resp = post(format!("/v0/projects/{slug}/reports"), body).await;
        assert_eq!(resp.status(), StatusCode::CREATED, "POST report {day}");
        let json: serde_json::Value = resp.json().await.expect("Failed to parse the report");
        alerts.extend(
            json.get("alerts")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|alert| alert.get("uuid").and_then(serde_json::Value::as_str))
                .map(ToOwned::to_owned),
        );
    }
    assert_eq!(alerts.len(), 1, "the outlier raised one alert");
    let alert = alerts.pop().expect("No alert");

    for (resource, name) in [("branches", "feature"), ("testbeds", "other")] {
        let resp = post(
            format!("/v0/projects/{slug}/{resource}"),
            serde_json::json!({ "name": name }),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::CREATED, "POST {resource}");
    }

    (user, slug, alert)
}

async fn patch(server: &TestServer, user: &TestUser, path: &str, body: serde_json::Value) {
    let resp = server
        .client
        .patch(server.api_url(path))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK, "PATCH {path}");
}

// A public project with no alerts passes, for a reader who is not signed in, with a
// badge any cache may keep.
#[tokio::test]
async fn badge_passes_a_project_with_no_alerts() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "badgenone@example.com").await;
    let org = server.create_org(&user, "Badge None Org").await;
    let project = server
        .create_project(&user, &org, "Badge None Project")
        .await;

    let resp = badge(&server, project.slug.as_ref(), "", None).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        header(&resp, http::header::CONTENT_TYPE),
        Some("image/svg+xml")
    );
    assert_eq!(
        header(&resp, http::header::X_CONTENT_TYPE_OPTIONS),
        Some("nosniff")
    );
    assert_eq!(
        header(&resp, http::header::ACCESS_CONTROL_ALLOW_ORIGIN),
        Some("*")
    );
    assert_eq!(
        header(&resp, http::header::CACHE_CONTROL),
        Some(PUBLIC_CACHE_CONTROL)
    );
    assert_eq!(
        resp.text().await.expect("Failed to read the badge"),
        PASSING_SVG
    );
}

// An active alert fails the project, and dismissing it passes the project again.
#[tokio::test]
async fn badge_fails_while_an_alert_is_active() {
    let server = TestServer::new().await;
    let (user, slug, alert) = alerting_project(&server, "active", "main").await;

    assert_eq!(badge_body(&server, &slug, "").await, FAILING_SVG);

    let path = format!("/v0/projects/{slug}/alerts/{alert}");
    patch(
        &server,
        &user,
        &path,
        serde_json::json!({ "status": "dismissed" }),
    )
    .await;
    assert_eq!(badge_body(&server, &slug, "").await, PASSING_SVG);

    patch(
        &server,
        &user,
        &path,
        serde_json::json!({ "status": "active" }),
    )
    .await;
    assert_eq!(badge_body(&server, &slug, "").await, FAILING_SVG);
}

// An active alert on an archived branch, testbed, or measure is not counted, as the
// console's Alerts tab does not count it.
#[tokio::test]
async fn badge_passes_when_the_alert_is_archived() {
    let server = TestServer::new().await;
    let (user, slug, _alert) = alerting_project(&server, "archived", "main").await;

    for resource in ["branches/main", "testbeds/localhost", "measures/latency"] {
        let path = format!("/v0/projects/{slug}/{resource}");
        patch(
            &server,
            &user,
            &path,
            serde_json::json!({ "archived": true }),
        )
        .await;
        assert_eq!(
            badge_body(&server, &slug, "").await,
            PASSING_SVG,
            "{resource}"
        );

        patch(
            &server,
            &user,
            &path,
            serde_json::json!({ "archived": false }),
        )
        .await;
        assert_eq!(
            badge_body(&server, &slug, "").await,
            FAILING_SVG,
            "{resource}"
        );
    }
}

// The branch and testbed narrow the badge to the alerts on them.
#[tokio::test]
async fn badge_narrows_to_a_branch_and_a_testbed() {
    let server = TestServer::new().await;
    let (_user, slug, _alert) = alerting_project(&server, "narrow", "main").await;

    for (query, expected) in [
        ("?branch=feature", PASSING_SVG),
        ("?branch=main", FAILING_SVG),
        ("?testbed=other", PASSING_SVG),
        ("?testbed=localhost", FAILING_SVG),
        ("?branch=main&testbed=other", PASSING_SVG),
        ("?branch=feature&testbed=localhost", PASSING_SVG),
        ("?branch=main&testbed=localhost", FAILING_SVG),
    ] {
        assert_eq!(badge_body(&server, &slug, query).await, expected, "{query}");
    }
}

// A branch is named by its name too, so a name that is not a slug, such as one with a
// slash, narrows the badge, written as is or percent-encoded.
#[tokio::test]
async fn badge_narrows_to_a_branch_by_its_name() {
    let server = TestServer::new().await;
    let (_user, slug, _alert) = alerting_project(&server, "named", "release/1.0").await;

    for (query, expected) in [
        ("?branch=release/1.0", FAILING_SVG),
        ("?branch=release%2F1.0", FAILING_SVG),
        ("?branch=feature", PASSING_SVG),
    ] {
        assert_eq!(badge_body(&server, &slug, query).await, expected, "{query}");
    }
}

// A branch or testbed that is not in the project is not found, as the API's usual error.
#[tokio::test]
async fn badge_refuses_a_branch_or_testbed_not_in_the_project() {
    let server = TestServer::new().await;
    let (_user, slug, _alert) = alerting_project(&server, "missing", "main").await;

    for query in [
        "?branch=nope",
        "?branch=release/2.0",
        "?testbed=nope",
        "?testbed=no%20such%20testbed",
    ] {
        let resp = badge(&server, &slug, query, None).await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND, "{query}");
        assert_eq!(
            header(&resp, http::header::CONTENT_TYPE),
            Some("application/json"),
            "{query}"
        );
    }
}

#[tokio::test]
async fn badge_refuses_an_unknown_project() {
    let server = TestServer::new().await;

    let resp = badge(&server, "nope", "", None).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        header(&resp, http::header::CONTENT_TYPE),
        Some("application/json")
    );
}

// A private project's badge is refused to a reader who is not signed in, as its alerts
// are, and is served to its project key with a badge only the reader may keep.
#[cfg(feature = "plus")]
#[tokio::test]
async fn badge_of_a_private_project() {
    use bencher_json::{JsonProjectKeyCreated, project::Visibility};
    use bencher_schema::schema;
    use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};

    let server = TestServer::new().await;
    let user = server.signup("Test User", "badgeprivate@example.com").await;
    let org = server.create_org(&user, "Badge Private Org").await;
    let project = server
        .create_project(&user, &org, "Badge Private Project")
        .await;
    {
        let mut conn = server.db_conn();
        diesel::update(schema::project::table.filter(schema::project::uuid.eq(project.uuid)))
            .set(schema::project::visibility.eq(Visibility::Private))
            .execute(&mut conn)
            .expect("Failed to update project visibility");
    }
    let slug: &str = project.slug.as_ref();

    let alerts = server
        .client
        .get(server.api_url(&format!("/v0/projects/{slug}/alerts")))
        .send()
        .await
        .expect("Request failed");
    let resp = badge(&server, slug, "", None).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    assert_eq!(resp.status(), alerts.status());
    assert_eq!(
        header(&resp, http::header::CONTENT_TYPE),
        Some("application/json")
    );

    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{slug}/keys")))
        .headers(server.fresh_session(&user))
        .json(&serde_json::json!({ "name": "badge-key" }))
        .send()
        .await
        .expect("Failed to create the project key");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let key: JsonProjectKeyCreated = resp.json().await.expect("Failed to parse the key");

    let resp = badge(&server, slug, "", Some(key.key.as_ref())).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        header(&resp, http::header::CACHE_CONTROL),
        Some(PRIVATE_CACHE_CONTROL)
    );
    assert_eq!(
        resp.text().await.expect("Failed to read the badge"),
        PASSING_SVG
    );
}

// A private project's badge is served to a signed in member, with a badge only the reader
// may keep, and is refused to a signed in reader who is not a member, as its alerts are.
#[cfg(feature = "plus")]
#[tokio::test]
async fn badge_of_a_private_project_for_a_signed_in_reader() {
    use bencher_json::project::Visibility;
    use bencher_schema::schema;
    use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};

    let server = TestServer::new().await;
    let (member, slug, _alert) = alerting_project(&server, "privatereader", "main").await;
    {
        let mut conn = server.db_conn();
        diesel::update(schema::project::table.filter(schema::project::slug.eq(&slug)))
            .set(schema::project::visibility.eq(Visibility::Private))
            .execute(&mut conn)
            .expect("Failed to update project visibility");
    }

    let resp = badge(&server, &slug, "", Some(&member.token)).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        header(&resp, http::header::CACHE_CONTROL),
        Some(PRIVATE_CACHE_CONTROL)
    );
    assert_eq!(
        resp.text().await.expect("Failed to read the badge"),
        FAILING_SVG
    );

    let outsider = server
        .signup("Outsider", "badgeprivateoutsider@example.com")
        .await;
    let alerts = server
        .client
        .get(server.api_url(&format!("/v0/projects/{slug}/alerts")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&outsider.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(alerts.status(), StatusCode::NOT_FOUND);
    let resp = badge(&server, &slug, "", Some(&outsider.token)).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        header(&resp, http::header::CONTENT_TYPE),
        Some("application/json")
    );
    let message = |body: serde_json::Value| body.get("message").cloned();
    assert_eq!(
        message(resp.json().await.expect("Failed to parse the refusal")),
        message(alerts.json().await.expect("Failed to parse the refusal")),
    );
}
