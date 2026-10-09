#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! `GET /v0/projects/{project}/console`: what the console's shell shows for a
//! project, in one request.

use bencher_api_tests::{TestServer, TestUser, helpers::get_project_id};
use bencher_json::{
    ProjectSlug,
    project::console_project::{JsonConsolePermissions, JsonConsoleProject},
};
use bencher_schema::schema;
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use http::StatusCode;

/// The status and body of the console project request, as `token` or signed out.
async fn get(server: &TestServer, slug: &ProjectSlug, token: Option<&str>) -> (StatusCode, String) {
    let slug: &str = slug.as_ref();
    let request = server
        .client
        .get(server.api_url(&format!("/v0/projects/{slug}/console")));
    let request = match token {
        Some(token) => request.header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(token),
        ),
        None => request,
    };
    let resp = request.send().await.expect("Request failed");
    let status = resp.status();
    (
        status,
        resp.text().await.expect("Failed to read the response"),
    )
}

async fn console(server: &TestServer, slug: &ProjectSlug, user: &TestUser) -> JsonConsoleProject {
    let (status, body) = get(server, slug, Some(&user.token)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    serde_json::from_str(&body).expect("Failed to parse response")
}

/// Give `user` the project `role` directly, since no endpoint grants one yet.
fn grant(server: &TestServer, user: &TestUser, slug: &ProjectSlug, role: &str) {
    let mut conn = server.db_conn();
    let user_id: i32 = schema::user::table
        .filter(schema::user::uuid.eq(user.uuid))
        .select(schema::user::id)
        .first(&mut conn)
        .expect("Failed to get user ID");
    diesel::insert_into(schema::project_role::table)
        .values((
            schema::project_role::user_id.eq(user_id),
            schema::project_role::project_id.eq(get_project_id(server, slug.as_ref())),
            schema::project_role::role.eq(role),
            schema::project_role::created.eq(0i64),
            schema::project_role::modified.eq(0i64),
        ))
        .execute(&mut conn)
        .expect("Failed to grant the project role");
}

/// The permissions named in `names`, and no others.
fn granted(names: &[&str]) -> JsonConsolePermissions {
    JsonConsolePermissions {
        view: names.contains(&"view"),
        create: names.contains(&"create"),
        edit: names.contains(&"edit"),
        delete: names.contains(&"delete"),
        manage: names.contains(&"manage"),
    }
}

// A member gets the project as the public endpoint serves it, its organization,
// and every permission the organization's Leader holds.
#[tokio::test]
async fn console_project_for_a_member() {
    let server = TestServer::new().await;
    let user = server.signup("Test User", "consoleproj@example.com").await;
    server.create_org(&user, "First Org").await;
    let org = server.create_org(&user, "Owning Org").await;
    let project = server.create_project(&user, &org, "Console Project").await;

    let json = console(&server, &project.slug, &user).await;

    assert_eq!(json.project.uuid, project.uuid);
    assert_eq!(json.project.name, project.name);
    assert_eq!(json.organization.uuid, org.uuid);
    assert_eq!(json.organization.name, org.name);
    assert_eq!(json.organization.slug, org.slug);
    assert_eq!(
        json.permissions,
        granted(&["view", "create", "edit", "delete", "manage"])
    );
    assert_eq!(json.active_alerts, 0);
}

// A signed in reader who is not a member reads a public project and may change
// nothing in it.
#[tokio::test]
async fn console_project_for_a_reader_of_a_public_project() {
    let server = TestServer::new().await;
    let owner = server.signup("Owner", "consoleprojowner@example.com").await;
    let reader = server
        .signup("Reader", "consoleprojreader@example.com")
        .await;
    let org = server.create_org(&owner, "Public Org").await;
    let project = server.create_project(&owner, &org, "Public Project").await;

    let json = console(&server, &project.slug, &reader).await;

    assert_eq!(json.project.uuid, project.uuid);
    assert_eq!(json.organization.name, org.name);
    assert_eq!(json.permissions, granted(&["view"]));
}

// Each project role grants exactly its own permissions.
#[tokio::test]
async fn console_project_permissions_follow_the_role() {
    let server = TestServer::new().await;
    let owner = server.signup("Owner", "consoleprojroles@example.com").await;
    let org = server.create_org(&owner, "Roles Org").await;
    let project = server.create_project(&owner, &org, "Roles Project").await;

    for (role, expected) in [
        ("viewer", granted(&["view"])),
        ("developer", granted(&["view", "create", "edit", "delete"])),
        (
            "maintainer",
            granted(&["view", "create", "edit", "delete", "manage"]),
        ),
    ] {
        let user = server
            .signup(role, &format!("consoleproj{role}@example.com"))
            .await;
        grant(&server, &user, &project.slug, role);
        assert_eq!(
            console(&server, &project.slug, &user).await.permissions,
            expected,
            "{role}"
        );
    }
}

// A private project is hidden from a signed in reader who cannot view it, the
// same as one that does not exist.
#[cfg(feature = "plus")]
#[tokio::test]
async fn console_project_refuses_a_private_project() {
    use bencher_json::project::Visibility;

    let server = TestServer::new().await;
    let owner = server.signup("Owner", "consoleprojpriv@example.com").await;
    let outsider = server
        .signup("Outsider", "consoleprojprivother@example.com")
        .await;
    let org = server.create_org(&owner, "Private Org").await;
    let project = server.create_project(&owner, &org, "Private Project").await;
    {
        let mut conn = server.db_conn();
        diesel::update(schema::project::table.filter(schema::project::uuid.eq(project.uuid)))
            .set(schema::project::visibility.eq(Visibility::Private))
            .execute(&mut conn)
            .expect("Failed to update project visibility");
    }

    let (status, _body) = get(&server, &project.slug, Some(&outsider.token)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let json = console(&server, &project.slug, &owner).await;
    assert_eq!(json.project.uuid, project.uuid);
}

// The console needs a signed in reader, even for a public project.
#[tokio::test]
async fn console_project_refuses_a_reader_who_is_not_signed_in() {
    let server = TestServer::new().await;
    let owner = server.signup("Owner", "consoleprojanon@example.com").await;
    let org = server.create_org(&owner, "Anon Org").await;
    let project = server.create_project(&owner, &org, "Anon Project").await;

    // Like every endpoint that takes a bearer token, a missing one is a bad request.
    let (status, _body) = get(&server, &project.slug, None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// The count is the active alerts the Alerts list counts, not every alert.
#[tokio::test]
async fn console_project_counts_the_active_alerts() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "consoleprojalerts@example.com")
        .await;
    let org = server.create_org(&user, "Alerts Org").await;
    let project = server.create_project(&user, &org, "Alerts Project").await;
    let slug: &str = project.slug.as_ref();

    let report = |day: usize, small: f64, large: f64| {
        let results = serde_json::json!({
            "bench": [
                { "parameters": { "size_mb": 16 }, "measures": { "latency": { "value": small } } },
                { "parameters": { "size_mb": 32 }, "measures": { "latency": { "value": large } } },
            ]
        })
        .to_string();
        serde_json::json!({
            "branch": "main",
            "testbed": "localhost",
            "start_time": format!("2024-01-{day:02}T00:00:00Z"),
            "end_time": format!("2024-01-{day:02}T00:01:00Z"),
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
        })
    };
    let mut alerts = Vec::new();
    for (day, (small, large)) in [
        (10.0, 100.0),
        (11.0, 101.0),
        (12.0, 102.0),
        (13.0, 103.0),
        (14.0, 104.0),
        (1_000.0, 10_000.0),
    ]
    .into_iter()
    .enumerate()
    {
        let resp = server
            .client
            .post(server.api_url(&format!("/v0/projects/{slug}/reports")))
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(&user.token),
            )
            .json(&report(day + 1, small, large))
            .send()
            .await
            .expect("Request failed");
        assert_eq!(resp.status(), StatusCode::CREATED);
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
    assert_eq!(alerts.len(), 2, "both variants jumped");

    let dismissed = alerts.first().expect("an alert to dismiss");
    let resp = server
        .client
        .patch(server.api_url(&format!("/v0/projects/{slug}/alerts/{dismissed}")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&serde_json::json!({ "status": "dismissed" }))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);

    assert_eq!(
        console(&server, &project.slug, &user).await.active_alerts,
        1
    );
}

// Kills a count that reads another project's plots, or none.
#[tokio::test]
async fn console_project_counts_the_pinned_plots() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "consoleprojplots@example.com")
        .await;
    let org = server.create_org(&user, "Plots Org").await;
    let project = server.create_project(&user, &org, "Plots Project").await;
    let other = server.create_project(&user, &org, "Other Project").await;
    let mut conn = server.db_conn();
    for (slug, rank) in [(&project.slug, 1i64), (&project.slug, 2), (&other.slug, 1)] {
        diesel::insert_into(schema::plot::table)
            .values((
                schema::plot::uuid.eq(bencher_json::PlotUuid::new().to_string()),
                schema::plot::project_id.eq(get_project_id(&server, slug.as_ref())),
                schema::plot::rank.eq(rank),
                schema::plot::lower_value.eq(false),
                schema::plot::upper_value.eq(false),
                schema::plot::lower_boundary.eq(false),
                schema::plot::upper_boundary.eq(false),
                schema::plot::x_axis.eq(0),
                schema::plot::y_axis.eq(0),
                schema::plot::window.eq(604_800i64),
                schema::plot::created.eq(0i64),
                schema::plot::modified.eq(0i64),
            ))
            .execute(&mut conn)
            .expect("Failed to insert a plot");
    }

    assert_eq!(console(&server, &project.slug, &user).await.plots, 2);
    assert_eq!(console(&server, &other.slug, &user).await.plots, 1);
}

// A failed count is the server's fault, never an alert the reader cannot find.
// Kills mapping the count's error to an Alert 404.
#[tokio::test]
async fn console_project_fails_as_a_server_error_when_the_count_fails() {
    let server = TestServer::new().await;
    let user = server
        .signup("Test User", "consoleprojcountfail@example.com")
        .await;
    let org = server.create_org(&user, "Count Fail Org").await;
    let project = server
        .create_project(&user, &org, "Count Fail Project")
        .await;
    {
        let mut conn = server.db_conn();
        diesel::sql_query("DROP TABLE alert")
            .execute(&mut conn)
            .expect("Failed to drop the alert table");
    }

    let (status, body) = get(&server, &project.slug, Some(&user.token)).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{body}");
}
