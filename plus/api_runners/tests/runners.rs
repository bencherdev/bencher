#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::indexing_slicing
)]
//! Integration tests for runner CRUD endpoints.

mod common;

use bencher_api_tests::TestServer;
use bencher_json::{JsonRunner, JsonRunnerToken, runner::JsonRunners};
use http::StatusCode;

// POST /v0/runners - admin can create runner
#[tokio::test]
async fn runners_create_as_admin() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "runneradmin@example.com").await;

    let body = serde_json::json!({
        "name": "My Test Runner"
    });

    let resp = server
        .client
        .post(server.api_url("/v0/runners"))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);
    let runner_token: JsonRunnerToken = resp.json().await.expect("Failed to parse response");
    assert!(!runner_token.uuid.to_string().is_empty());
    // Token should start with prefix
    let token_str: &str = runner_token.token.as_ref();
    assert!(token_str.starts_with("bencher_runner_"));
}

// POST /v0/runners - non-admin cannot create runner
#[tokio::test]
async fn runners_create_forbidden_for_non_admin() {
    let server = TestServer::new().await;
    let _admin = server.signup("Admin", "adminrun1@example.com").await;
    let user = server.signup("User", "userrun1@example.com").await;

    let body = serde_json::json!({
        "name": "My Runner"
    });

    let resp = server
        .client
        .post(server.api_url("/v0/runners"))
        .header("Authorization", server.bearer(&user.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// POST /v0/runners - create with custom slug
#[tokio::test]
async fn runners_create_with_slug() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "runneradmin2@example.com").await;

    let body = serde_json::json!({
        "name": "My Test Runner",
        "slug": "custom-runner-slug"
    });

    let resp = server
        .client
        .post(server.api_url("/v0/runners"))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::CREATED);

    // Now get the runner by slug
    let resp = server
        .client
        .get(server.api_url("/v0/runners/custom-runner-slug"))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let runner: JsonRunner = resp.json().await.expect("Failed to parse response");
    assert_eq!(runner.name.as_ref(), "My Test Runner");
}

// GET /v0/runners - admin can list runners
#[tokio::test]
async fn runners_list_as_admin() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "runneradmin3@example.com").await;

    // Create a runner first
    let body = serde_json::json!({
        "name": "List Test Runner"
    });
    server
        .client
        .post(server.api_url("/v0/runners"))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    // List runners
    let resp = server
        .client
        .get(server.api_url("/v0/runners"))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let runners: JsonRunners = resp.json().await.expect("Failed to parse response");
    assert!(!runners.0.is_empty());
}

// GET /v0/runners/{runner} - get by UUID
#[tokio::test]
async fn runners_get_by_uuid() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "runneradmin4@example.com").await;

    // Create a runner
    let body = serde_json::json!({
        "name": "Get Test Runner"
    });
    let resp = server
        .client
        .post(server.api_url("/v0/runners"))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    let runner_token: JsonRunnerToken = resp.json().await.expect("Failed to parse response");

    // Get by UUID
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/runners/{}", runner_token.uuid)))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let runner: JsonRunner = resp.json().await.expect("Failed to parse response");
    assert_eq!(runner.uuid, runner_token.uuid);
    assert_eq!(runner.name.as_ref(), "Get Test Runner");
}

// PATCH /v0/runners/{runner} - update runner name
#[tokio::test]
async fn runners_update_name() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "runneradmin5@example.com").await;

    // Create a runner
    let body = serde_json::json!({
        "name": "Original Name"
    });
    let resp = server
        .client
        .post(server.api_url("/v0/runners"))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    let runner_token: JsonRunnerToken = resp.json().await.expect("Failed to parse response");

    // Update name
    let body = serde_json::json!({
        "name": "Updated Name"
    });
    let resp = server
        .client
        .patch(server.api_url(&format!("/v0/runners/{}", runner_token.uuid)))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let runner: JsonRunner = resp.json().await.expect("Failed to parse response");
    assert_eq!(runner.name.as_ref(), "Updated Name");
}

// PATCH /v0/runners/{runner} - archive runner
#[tokio::test]
async fn runners_archive() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "runneradmin7@example.com").await;

    // Create a runner
    let body = serde_json::json!({
        "name": "Archive Test Runner"
    });
    let resp = server
        .client
        .post(server.api_url("/v0/runners"))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    let runner_token: JsonRunnerToken = resp.json().await.expect("Failed to parse response");

    // Archive the runner
    let body = serde_json::json!({
        "archived": true
    });
    let resp = server
        .client
        .patch(server.api_url(&format!("/v0/runners/{}", runner_token.uuid)))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let runner: JsonRunner = resp.json().await.expect("Failed to parse response");
    assert!(runner.archived.is_some());

    // Archived runner should not appear in list (by default)
    let resp = server
        .client
        .get(server.api_url("/v0/runners"))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");

    let runners: JsonRunners = resp.json().await.expect("Failed to parse response");
    let found = runners.0.iter().any(|r| r.uuid == runner_token.uuid);
    assert!(!found, "Archived runner should not appear in list");
}

// GET /v0/runners?archived=true - list includes archived
#[tokio::test]
async fn runners_list_with_archived() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "runneradmin8@example.com").await;

    // Create and archive a runner
    let body = serde_json::json!({
        "name": "Archived Runner"
    });
    let resp = server
        .client
        .post(server.api_url("/v0/runners"))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    let runner_token: JsonRunnerToken = resp.json().await.expect("Failed to parse response");

    let body = serde_json::json!({
        "archived": true
    });
    server
        .client
        .patch(server.api_url(&format!("/v0/runners/{}", runner_token.uuid)))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    // List with archived=true
    let resp = server
        .client
        .get(server.api_url("/v0/runners?archived=true"))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");

    let runners: JsonRunners = resp.json().await.expect("Failed to parse response");
    let found = runners.0.iter().any(|r| r.uuid == runner_token.uuid);
    assert!(found, "Archived runner should appear when archived=true");
}

// GET /v0/runners - X-Total-Count header reflects the number of runners
#[tokio::test]
async fn runners_total_count_header() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "runnertotalcount@example.com").await;

    // Create 2 runners
    let body1 = serde_json::json!({ "name": "Count Runner 1" });
    server
        .client
        .post(server.api_url("/v0/runners"))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body1)
        .send()
        .await
        .expect("Request failed");

    let body2 = serde_json::json!({ "name": "Count Runner 2" });
    server
        .client
        .post(server.api_url("/v0/runners"))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body2)
        .send()
        .await
        .expect("Request failed");

    // List runners and check X-Total-Count header
    let resp = server
        .client
        .get(server.api_url("/v0/runners"))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let total_count = resp
        .headers()
        .get("X-Total-Count")
        .expect("Missing X-Total-Count header")
        .to_str()
        .expect("Invalid header value");
    let count: u64 = total_count.parse().expect("Invalid count");
    assert!(
        count >= 2,
        "Expected at least 2 runners in X-Total-Count, got {count}"
    );
}

// DELETE runner with active jobs should fail due to FK constraint (ON DELETE RESTRICT)
#[tokio::test]
async fn runner_delete_restricted_by_fk() {
    use bencher_schema::schema;
    use common::{
        associate_runner_spec, create_runner, create_test_report, get_project_id, get_runner_id,
        insert_test_job, insert_test_spec,
    };
    use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};

    let server = TestServer::new().await;
    let admin = server.signup("Admin", "fkconstraint@example.com").await;
    let org = server.create_org(&admin, "FK Constraint Org").await;
    let project = server
        .create_project(&admin, &org, "FK Constraint Project")
        .await;

    let (_, spec_id) = insert_test_spec(&server);
    let runner = create_runner(&server, &admin.token, "FK Constraint Runner").await;
    let runner_token: &str = runner.token.as_ref();
    let runner_id = get_runner_id(&server, runner.uuid);
    associate_runner_spec(&server, runner_id, spec_id);

    let project_id = get_project_id(&server, project.slug.as_ref());
    let report_id = create_test_report(&server, project_id);
    let _job_uuid = insert_test_job(&server, report_id, spec_id);

    // Claim the job so runner_id is set
    let body = serde_json::json!({ "poll_timeout": 5 });
    let resp = server
        .client
        .post(server.api_url(&format!("/v0/runners/{}/jobs", runner.uuid)))
        .header("Authorization", format!("Bearer {runner_token}"))
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let claimed: Option<bencher_json::JsonJob> = resp.json().await.expect("Failed to parse");
    assert!(claimed.is_some());

    // Try to delete the runner directly in the DB — should fail due to ON DELETE RESTRICT
    let runner_id = get_runner_id(&server, runner.uuid);
    let mut conn = server.db_conn();
    let result = diesel::delete(schema::runner::table.filter(schema::runner::id.eq(runner_id)))
        .execute(&mut conn);

    assert!(
        result.is_err(),
        "Expected FK constraint to prevent runner deletion while jobs reference it"
    );
}

// Creating two runners with the same name should produce a slug collision
#[tokio::test]
async fn duplicate_runner_name() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "duprunner@example.com").await;

    // Create first runner
    let body = serde_json::json!({ "name": "Duplicate Runner" });
    let resp = server
        .client
        .post(server.api_url("/v0/runners"))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Create second runner with the same name — should fail due to slug UNIQUE constraint
    let resp = server
        .client
        .post(server.api_url("/v0/runners"))
        .header("Authorization", server.bearer(&admin.token))
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(
        resp.status(),
        StatusCode::CONFLICT,
        "Duplicate runner name should produce a slug collision"
    );
}

// GET /v0/runners - sorting by name ascending
#[tokio::test]
async fn runners_list_sort_name_asc() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "rsortasc@example.com").await;

    // Create runners with names that sort in a known order
    for name in ["Alpha Runner", "Beta Runner", "Gamma Runner"] {
        let body = serde_json::json!({ "name": name });
        let resp = server
            .client
            .post(server.api_url("/v0/runners"))
            .header("Authorization", server.bearer(&admin.token))
            .json(&body)
            .send()
            .await
            .expect("Request failed");
        assert_eq!(resp.status(), StatusCode::CREATED);
    }

    // List with sort=name and direction=asc
    let resp = server
        .client
        .get(server.api_url("/v0/runners?sort=name&direction=asc"))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let runners: JsonRunners = resp.json().await.expect("Failed to parse response");
    assert!(runners.0.len() >= 3);

    // Verify ascending name order
    let names: Vec<&str> = runners.0.iter().map(|r| r.name.as_ref()).collect();
    for i in 1..names.len() {
        assert!(
            names[i - 1] <= names[i],
            "Expected ascending order, but {:?} > {:?}",
            names[i - 1],
            names[i]
        );
    }
}

// GET /v0/runners - sorting by name descending
#[tokio::test]
async fn runners_list_sort_name_desc() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "rsortdesc@example.com").await;

    for name in ["Alpha Runner", "Beta Runner", "Gamma Runner"] {
        let body = serde_json::json!({ "name": name });
        let resp = server
            .client
            .post(server.api_url("/v0/runners"))
            .header("Authorization", server.bearer(&admin.token))
            .json(&body)
            .send()
            .await
            .expect("Request failed");
        assert_eq!(resp.status(), StatusCode::CREATED);
    }

    let resp = server
        .client
        .get(server.api_url("/v0/runners?sort=name&direction=desc"))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let runners: JsonRunners = resp.json().await.expect("Failed to parse response");
    assert!(runners.0.len() >= 3);

    let names: Vec<&str> = runners.0.iter().map(|r| r.name.as_ref()).collect();
    for i in 1..names.len() {
        assert!(
            names[i - 1] >= names[i],
            "Expected descending order, but {:?} < {:?}",
            names[i - 1],
            names[i]
        );
    }
}

// GET /v0/runners - pagination with per_page and page
#[tokio::test]
async fn runners_list_pagination() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "rpaginate@example.com").await;

    // Create 3 runners
    for i in 1..=3 {
        let body = serde_json::json!({ "name": format!("Page Runner {i}") });
        let resp = server
            .client
            .post(server.api_url("/v0/runners"))
            .header("Authorization", server.bearer(&admin.token))
            .json(&body)
            .send()
            .await
            .expect("Request failed");
        assert_eq!(resp.status(), StatusCode::CREATED);
    }

    // Request first page with per_page=2
    let resp = server
        .client
        .get(server.api_url("/v0/runners?per_page=2&page=1"))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let runners: JsonRunners = resp.json().await.expect("Failed to parse response");
    assert_eq!(runners.0.len(), 2, "First page should have 2 runners");

    // Request second page
    let resp = server
        .client
        .get(server.api_url("/v0/runners?per_page=2&page=2"))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let runners_page2: JsonRunners = resp.json().await.expect("Failed to parse response");
    assert_eq!(runners_page2.0.len(), 1, "Second page should have 1 runner");

    // Pages should contain different runners
    let page1_uuids: Vec<_> = runners.0.iter().map(|r| r.uuid).collect();
    let page2_uuids: Vec<_> = runners_page2.0.iter().map(|r| r.uuid).collect();
    for uuid in &page2_uuids {
        assert!(
            !page1_uuids.contains(uuid),
            "Page 2 runner should not appear on page 1"
        );
    }
}

// GET /v0/runners - search filtering
#[tokio::test]
async fn runners_list_search() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "rsearch@example.com").await;

    // Create runners with distinct names
    for name in ["Search Alpha", "Search Beta", "Other Runner"] {
        let body = serde_json::json!({ "name": name });
        let resp = server
            .client
            .post(server.api_url("/v0/runners"))
            .header("Authorization", server.bearer(&admin.token))
            .json(&body)
            .send()
            .await
            .expect("Request failed");
        assert_eq!(resp.status(), StatusCode::CREATED);
    }

    // Search for "Search"
    let resp = server
        .client
        .get(server.api_url("/v0/runners?search=Search"))
        .header("Authorization", server.bearer(&admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let runners: JsonRunners = resp.json().await.expect("Failed to parse response");
    assert_eq!(runners.0.len(), 2, "Search should match 2 runners");
    for runner in &runners.0 {
        assert!(
            runner.name.as_ref().contains("Search"),
            "All results should contain 'Search', got: {:?}",
            runner.name
        );
    }
}

// A runner token should be rejected on user endpoints (e.g., project listing)
#[tokio::test]
async fn runner_token_rejected_on_user_endpoint() {
    let server = TestServer::new().await;
    let admin = server.signup("Admin", "runnertokusr@example.com").await;

    let runner = common::create_runner(&server, &admin.token, "Cross Auth Runner").await;
    let runner_token: &str = runner.token.as_ref();

    // Use runner token on a user endpoint that requires authentication
    let resp = server
        .client
        .get(server.api_url("/v0/users"))
        .header("Authorization", format!("Bearer {runner_token}"))
        .send()
        .await
        .expect("Request failed");

    // Runner tokens should not work on user endpoints
    assert_ne!(
        resp.status(),
        StatusCode::OK,
        "Runner token should not authenticate on user endpoints"
    );
}
