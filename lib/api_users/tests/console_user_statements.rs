#![expect(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::tests_outside_test_module,
    reason = "integration test file"
)]
//! `GET /v0/console/users/{user}` answers in a fixed number of statements.
//!
//! Its own test binary, since the statement count is process wide.

use std::{collections::HashMap, sync::Mutex};

use bencher_api_tests::{
    TestServer, TestUser,
    helpers::{create_test_report, get_project_id},
};
use diesel::connection::{Instrumentation, InstrumentationEvent, set_default_instrumentation};
use http::StatusCode;

static STATEMENTS: Mutex<Vec<String>> = Mutex::new(Vec::new());

#[expect(clippy::unnecessary_wraps, reason = "the signature diesel asks for")]
fn record_statements() -> Option<Box<dyn Instrumentation>> {
    Some(Box::new(|event: InstrumentationEvent<'_>| {
        if let InstrumentationEvent::StartQuery { query, .. } = event {
            STATEMENTS
                .lock()
                .expect("Failed to record a statement")
                .push(shape(&query.to_string()));
        }
    }))
}

/// A statement without its binds, its lists of placeholders collapsed to one.
fn shape(statement: &str) -> String {
    let mut shape = statement
        .split(" -- binds:")
        .next()
        .unwrap_or_default()
        .to_owned();
    while shape.contains("?, ?") {
        shape = shape.replace("?, ?", "?");
    }
    shape
}

/// The fewest statements one bootstrap request runs, of three after a first request
/// that opens whatever later ones reuse: a statement the server runs in the
/// background can only add to one of them.
async fn statements(server: &TestServer, user: &TestUser) -> Vec<String> {
    request(server, user).await;
    let mut fewest: Option<Vec<String>> = None;
    for _ in 0..3 {
        STATEMENTS
            .lock()
            .expect("Failed to clear the statements")
            .clear();
        request(server, user).await;
        let statements =
            std::mem::take(&mut *STATEMENTS.lock().expect("Failed to read the statements"));
        if fewest
            .as_ref()
            .is_none_or(|fewest| statements.len() < fewest.len())
        {
            fewest = Some(statements);
        }
    }
    fewest.unwrap_or_default()
}

async fn request(server: &TestServer, user: &TestUser) {
    let slug: &str = user.slug.as_ref();
    let resp = server
        .client
        .get(server.api_url(&format!("/v0/console/users/{slug}")))
        .header(
            bencher_json::AUTHORIZATION,
            bencher_json::bearer_header(&user.token),
        )
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), StatusCode::OK, "the bootstrap reads");
}

/// The statements one list runs more often than the other, as a count each.
fn difference(few: &[String], many: &[String]) -> HashMap<String, isize> {
    let mut counts = HashMap::<String, isize>::new();
    for statement in few {
        *counts.entry(statement.clone()).or_default() += 1;
    }
    for statement in many {
        *counts.entry(statement.clone()).or_default() -= 1;
    }
    counts.retain(|_, count| *count != 0);
    counts
}

// Kills a bootstrap that reads each project's newest report or alerts with a
// statement of its own.
#[tokio::test]
async fn console_user_statements_do_not_grow_with_projects() {
    // Set before the server opens its connections, which take the default when made.
    set_default_instrumentation(record_statements).expect("Failed to record statements");
    let server = TestServer::new().await;
    let reader = server
        .signup("Reader", "consoleuserstatements@example.com")
        .await;
    let org = server.create_org(&reader, "Statements Org").await;
    let add_project = async |name: &str| {
        let project = server.create_project(&reader, &org, name).await;
        create_test_report(&server, get_project_id(&server, project.slug.as_ref()));
    };
    add_project("Project 0").await;
    let few = statements(&server, &reader).await;

    for n in 1..8 {
        add_project(&format!("Project {n}")).await;
    }
    let many = statements(&server, &reader).await;

    assert_eq!(few.len(), many.len(), "{:#?}", difference(&few, &many));
}
