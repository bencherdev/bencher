#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! `GET /v0/console/users/{user}`: what the console shows about its reader, in
//! one request.

use std::collections::{HashMap, HashSet};

use bencher_api_tests::{
    TestProject, TestServer, TestUser,
    helpers::{base_timestamp, get_project_id},
};
use bencher_json::{
    BmfVersion, DateTime, DateTimeMillis, OrganizationUuid, ProjectUuid,
    project::console_project::JsonConsoleProject,
    user::console_user::{JsonConsoleUser, JsonConsoleUserProject},
};
use bencher_schema::schema;
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use http::StatusCode;

/// 2024-01-06T00:00:00Z, the newest of [`raise_two_alerts`]'s reports.
const JAN_6: i64 = 1_704_499_200;
/// 2024-03-01T12:30:00Z
const MAR_1_NOON_THIRTY: i64 = 1_709_296_200;

/// The status and body of the reader bootstrap for `user`, read as `token`.
async fn get(server: &TestServer, user: &TestUser, token: &str) -> (StatusCode, String) {
    let slug: &str = user.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/console/users/{slug}")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(token),
        )
        .send()
        .await
        .expect("Request failed");
    let status = resp.status();
    (
        status,
        resp.text().await.expect("Failed to read the response"),
    )
}

async fn console(server: &TestServer, user: &TestUser) -> JsonConsoleUser {
    let (status, body) = get(server, user, &user.token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    serde_json::from_str(&body).expect("Failed to parse response")
}

fn projects_by_uuid(json: &JsonConsoleUser) -> HashMap<ProjectUuid, &JsonConsoleUserProject> {
    json.projects
        .iter()
        .map(|project| (project.uuid, project))
        .collect()
}

fn set_bmf_version(server: &TestServer, project: &TestProject, version: BmfVersion) {
    let mut conn = server.db_conn();
    diesel::update(schema::project::table.filter(schema::project::uuid.eq(project.uuid)))
        .set(schema::project::bmf_version.eq(version))
        .execute(&mut conn)
        .expect("Failed to set the project's BMF version");
}

/// A report on `branch` that ends at `end`, checked by a threshold on latency.
async fn post_report(
    server: &TestServer,
    user: &TestUser,
    project: &TestProject,
    branch: &str,
    end: &str,
    latency: (f64, f64),
) -> usize {
    let (small, large) = latency;
    let results = serde_json::json!({
        "bench": [
            { "parameters": { "size_mb": 16 }, "measures": { "latency": { "value": small } } },
            { "parameters": { "size_mb": 32 }, "measures": { "latency": { "value": large } } },
        ]
    })
    .to_string();
    let body = serde_json::json!({
        "branch": branch,
        "testbed": "localhost",
        "start_time": end,
        "end_time": end,
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
    let slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/projects/{slug}/reports")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED, "the report posts");
    let json: serde_json::Value = resp.json().await.expect("Failed to parse the report");
    json.get("alerts")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len)
}

/// Six reports on `branch`, the last of which raises an alert on both lines.
async fn raise_two_alerts(
    server: &TestServer,
    user: &TestUser,
    project: &TestProject,
    branch: &str,
) {
    let mut alerts = 0;
    for (day, latency) in [
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
        let end = format!("2024-01-{:02}T00:00:00Z", day + 1);
        alerts += post_report(server, user, project, branch, &end, latency).await;
    }
    assert_eq!(alerts, 2, "both lines jumped");
}

async fn dismiss_one(server: &TestServer, user: &TestUser, project: &TestProject) {
    let alert: String = schema::alert::table
        .filter(schema::alert::project_id.eq(get_project_id(server, project.slug.as_ref())))
        .select(schema::alert::uuid)
        .first(&mut server.db_conn())
        .expect("Failed to get an alert");
    let slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .patch(server.api_url(&format!("/v0/projects/{slug}/alerts/{alert}")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .json(&serde_json::json!({ "status": "dismissed" }))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK, "the alert dismisses");
}

/// Archive every `dimension` of the project, so none of its alerts shows by default.
fn archive(server: &TestServer, project: &TestProject, dimension: &str) {
    let mut conn = server.db_conn();
    let project_id = get_project_id(server, project.slug.as_ref());
    let now = base_timestamp();
    let archived = match dimension {
        "branch" => {
            diesel::update(schema::branch::table.filter(schema::branch::project_id.eq(project_id)))
                .set(schema::branch::archived.eq(now))
                .execute(&mut conn)
        },
        "testbed" => diesel::update(
            schema::testbed::table.filter(schema::testbed::project_id.eq(project_id)),
        )
        .set(schema::testbed::archived.eq(now))
        .execute(&mut conn),
        _ => diesel::update(
            schema::measure::table.filter(schema::measure::project_id.eq(project_id)),
        )
        .set(schema::measure::archived.eq(now))
        .execute(&mut conn),
    }
    .expect("Failed to archive");
    assert!(archived > 0, "{dimension}");
}

async fn console_project(
    server: &TestServer,
    user: &TestUser,
    project: &TestProject,
) -> JsonConsoleProject {
    let slug: &str = project.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/console/projects/{slug}")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK, "the project shell reads");
    resp.json().await.expect("Failed to parse response")
}

// Kills a bootstrap that lists every organization or project on the server, lists
// projects by project role instead of membership, keeps deleted organizations or
// projects, or names the wrong organization or version of a project.
#[tokio::test]
async fn console_user_lists_the_readers_organizations_and_projects() {
    let server = TestServer::new().await;
    let reader = server.signup("Reader", "consoleuser@example.com").await;
    let other = server.signup("Other", "consoleuserother@example.com").await;
    let first = server.create_org(&reader, "First Org").await;
    let second = server.create_org(&reader, "Second Org").await;
    let alpha = server.create_project(&reader, &first, "Alpha").await;
    let beta = server.create_project(&reader, &second, "Beta").await;
    let gone = server.create_project(&reader, &second, "Gone").await;
    let third = server.create_org(&reader, "Third Org").await;
    server.create_project(&reader, &third, "Doomed").await;
    let third_slug: &str = third.slug.as_ref();
    let resp = server
        .client
        .delete(server.api_url(&format!("/v0/organizations/{third_slug}")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&reader.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert!(resp.status().is_success(), "the organization deletes");
    set_bmf_version(&server, &beta, BmfVersion::V1);
    {
        let mut conn = server.db_conn();
        diesel::update(schema::project::table.filter(schema::project::uuid.eq(gone.uuid)))
            .set(schema::project::deleted.eq(base_timestamp()))
            .execute(&mut conn)
            .expect("Failed to delete the project");
    }
    // A project the reader holds a role on, in an organization they do not belong to.
    let elsewhere = server.create_org(&other, "Elsewhere Org").await;
    let foreign = server.create_project(&other, &elsewhere, "Foreign").await;
    {
        let mut conn = server.db_conn();
        let reader_id: i32 = schema::user::table
            .filter(schema::user::uuid.eq(reader.uuid))
            .select(schema::user::id)
            .first(&mut conn)
            .expect("Failed to get the reader's ID");
        diesel::insert_into(schema::project_role::table)
            .values((
                schema::project_role::user_id.eq(reader_id),
                schema::project_role::project_id.eq(get_project_id(&server, foreign.slug.as_ref())),
                schema::project_role::role.eq("viewer"),
                schema::project_role::created.eq(0i64),
                schema::project_role::modified.eq(0i64),
            ))
            .execute(&mut conn)
            .expect("Failed to grant the project role");
    }

    let json = console(&server, &reader).await;

    assert_eq!(json.uuid, reader.uuid);
    assert_eq!(json.name, reader.name);
    assert_eq!(json.slug, reader.slug);
    // Signing up made the reader an organization of their own.
    let organizations = json
        .organizations
        .iter()
        .map(|org| (org.slug.to_string(), (org.name.to_string(), org.uuid)))
        .collect::<HashMap<_, _>>();
    assert_eq!(
        organizations.keys().cloned().collect::<HashSet<_>>(),
        HashSet::from(["reader", "first-org", "second-org"].map(ToOwned::to_owned))
    );
    for org in [&first, &second] {
        assert_eq!(
            organizations.get(AsRef::<str>::as_ref(&org.slug)),
            Some(&(org.name.to_string(), org.uuid))
        );
    }
    let projects = json
        .projects
        .iter()
        .map(|project| {
            (
                project.uuid,
                project.name.to_string(),
                project.slug.to_string(),
                project.organization,
                project.bmf_version,
            )
        })
        .collect::<HashSet<_>>();
    let expected: HashSet<(ProjectUuid, String, String, OrganizationUuid, BmfVersion)> =
        HashSet::from([
            (
                alpha.uuid,
                alpha.name.to_string(),
                alpha.slug.to_string(),
                first.uuid,
                BmfVersion::V0,
            ),
            (
                beta.uuid,
                beta.name.to_string(),
                beta.slug.to_string(),
                second.uuid,
                BmfVersion::V1,
            ),
        ]);
    assert_eq!(projects, expected);
}

// Kills a bootstrap that is open to any signed in user, or that an admin reads as
// their own memberships instead of the user's.
#[tokio::test]
async fn console_user_is_only_for_the_reader_or_an_admin() {
    let server = TestServer::new().await;
    // The first user to sign up is the server admin.
    let admin = server.signup("Admin", "consoleuseradmin@example.com").await;
    let reader = server
        .signup("Reader", "consoleuserreader@example.com")
        .await;
    let other = server
        .signup("Other", "consoleuserstranger@example.com")
        .await;
    server.create_org(&admin, "Admin Org").await;
    let org = server.create_org(&reader, "Reader Org").await;

    let (status, _body) = get(&server, &reader, &other.token).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "another user");

    let (status, body) = get(&server, &reader, &admin.token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let json: JsonConsoleUser = serde_json::from_str(&body).expect("Failed to parse response");
    assert_eq!(json.uuid, reader.uuid);
    let organizations = json
        .organizations
        .iter()
        .map(|org| (org.slug.to_string(), org.uuid))
        .collect::<HashMap<_, _>>();
    assert_eq!(organizations.len(), 2, "{organizations:?}");
    assert!(organizations.contains_key("reader"), "{organizations:?}");
    assert_eq!(
        organizations.get(AsRef::<str>::as_ref(&org.slug)),
        Some(&org.uuid)
    );
}

// Kills a newest report read by insertion order or creation time instead of end
// time, or read across projects; an alert count that is not per project, counts
// dismissed alerts or alerts on an archived branch, testbed, or measure, or
// differs from the count the console's project shell shows; and a project with
// no reports that drops the field instead of reading null.
#[tokio::test]
async fn console_user_reads_each_projects_newest_report_and_active_alerts() {
    let server = TestServer::new().await;
    let reader = server
        .signup("Reader", "consoleusernewest@example.com")
        .await;
    let org = server.create_org(&reader, "Newest Org").await;
    let alerting = server.create_project(&reader, &org, "Alerting").await;
    let quiet = server.create_project(&reader, &org, "Quiet").await;
    let unrun = server.create_project(&reader, &org, "Unrun").await;

    raise_two_alerts(&server, &reader, &alerting, "main").await;
    // An older report posted last.
    post_report(
        &server,
        &reader,
        &alerting,
        "main",
        "2023-12-25T00:00:00Z",
        (10.0, 100.0),
    )
    .await;
    dismiss_one(&server, &reader, &alerting).await;

    let mut archived = Vec::new();
    for dimension in ["branch", "testbed", "measure"] {
        let name = format!("Archived {dimension}");
        let project = server.create_project(&reader, &org, &name).await;
        raise_two_alerts(&server, &reader, &project, "main").await;
        archive(&server, &project, dimension);
        archived.push((dimension, project));
    }

    post_report(
        &server,
        &reader,
        &quiet,
        "main",
        "2024-03-01T12:30:00Z",
        (10.0, 100.0),
    )
    .await;

    let (status, body) = get(&server, &reader, &reader.token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let raw: serde_json::Value = serde_json::from_str(&body).expect("Failed to parse response");
    let unrun_json = raw
        .get("projects")
        .and_then(serde_json::Value::as_array)
        .and_then(|projects| {
            projects.iter().find(|project| {
                project.get("uuid").and_then(serde_json::Value::as_str)
                    == Some(unrun.uuid.to_string().as_str())
            })
        })
        .expect("The project with no reports");
    assert!(
        unrun_json
            .get("last_report")
            .is_some_and(serde_json::Value::is_null),
        "{unrun_json}"
    );

    let json: JsonConsoleUser = serde_json::from_str(&body).expect("Failed to parse response");
    let projects = projects_by_uuid(&json);
    let millis = |seconds: i64| {
        DateTimeMillis::from(DateTime::try_from(seconds).expect("Failed to read the time"))
    };
    let read = |project: &TestProject| {
        let project = projects.get(&project.uuid).expect("The project");
        (project.last_report, project.active_alerts)
    };
    assert_eq!(read(&alerting), (Some(millis(JAN_6)), 1));
    for (dimension, project) in &archived {
        assert_eq!(read(project), (Some(millis(JAN_6)), 0), "{dimension}");
    }
    assert_eq!(read(&quiet), (Some(millis(MAR_1_NOON_THIRTY)), 0));
    assert_eq!(read(&unrun), (None, 0));

    for project in [&alerting]
        .into_iter()
        .chain(archived.iter().map(|(_, project)| project))
    {
        assert_eq!(
            read(project).1,
            console_project(&server, &reader, project)
                .await
                .active_alerts,
        );
    }
}
