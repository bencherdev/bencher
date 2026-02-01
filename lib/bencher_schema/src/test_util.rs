//! Test utilities for `bencher_schema` unit tests.
//!
//! This module provides helper functions for setting up test databases
//! and creating test fixtures for unit testing.

use diesel::{
    Connection as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _, SqliteConnection,
};

use crate::{run_migrations, schema};

/// Create an in-memory `SQLite` database with migrations applied.
pub fn setup_test_db() -> SqliteConnection {
    let mut conn =
        SqliteConnection::establish(":memory:").expect("Failed to create in-memory database");
    run_migrations(&mut conn).expect("Failed to run migrations");
    conn
}

/// IDs returned from creating base entities.
#[derive(Debug, Clone, Copy)]
pub struct BaseEntityIds {
    pub organization_id: i32,
    pub project_id: i32,
}

/// Create the base entities (organization, project) required for most tests.
pub fn create_base_entities(conn: &mut SqliteConnection) -> BaseEntityIds {
    diesel::insert_into(schema::organization::table)
        .values((
            schema::organization::uuid.eq("00000000-0000-0000-0000-000000000001"),
            schema::organization::name.eq("Test Org"),
            schema::organization::slug.eq("test-org"),
            schema::organization::created.eq(0i64),
            schema::organization::modified.eq(0i64),
        ))
        .execute(conn)
        .expect("Failed to insert organization");

    diesel::insert_into(schema::project::table)
        .values((
            schema::project::uuid.eq("00000000-0000-0000-0000-000000000002"),
            schema::project::organization_id.eq(1),
            schema::project::name.eq("Test Project"),
            schema::project::slug.eq("test-project"),
            schema::project::visibility.eq(0),
            schema::project::created.eq(0i64),
            schema::project::modified.eq(0i64),
        ))
        .execute(conn)
        .expect("Failed to insert project");

    BaseEntityIds {
        organization_id: 1,
        project_id: 1,
    }
}

/// IDs returned from creating a branch with head.
#[derive(Debug, Clone, Copy)]
pub struct BranchIds {
    pub branch_id: i32,
    pub head_id: i32,
}

/// Create a branch and head for testing.
pub fn create_branch_with_head(
    conn: &mut SqliteConnection,
    project_id: i32,
    branch_uuid: &str,
    branch_name: &str,
    branch_slug: &str,
    head_uuid: &str,
) -> BranchIds {
    // First create branch without head_id
    diesel::insert_into(schema::branch::table)
        .values((
            schema::branch::uuid.eq(branch_uuid),
            schema::branch::project_id.eq(project_id),
            schema::branch::name.eq(branch_name),
            schema::branch::slug.eq(branch_slug),
            schema::branch::created.eq(0i64),
            schema::branch::modified.eq(0i64),
        ))
        .execute(conn)
        .expect("Failed to insert branch");

    let branch_id: i32 = schema::branch::table
        .filter(schema::branch::uuid.eq(branch_uuid))
        .select(schema::branch::id)
        .first(conn)
        .expect("Failed to get branch id");

    diesel::insert_into(schema::head::table)
        .values((
            schema::head::uuid.eq(head_uuid),
            schema::head::branch_id.eq(branch_id),
            schema::head::created.eq(0i64),
        ))
        .execute(conn)
        .expect("Failed to insert head");

    let head_id: i32 = schema::head::table
        .filter(schema::head::uuid.eq(head_uuid))
        .select(schema::head::id)
        .first(conn)
        .expect("Failed to get head id");

    // Update branch to point to head
    diesel::update(schema::branch::table.filter(schema::branch::id.eq(branch_id)))
        .set(schema::branch::head_id.eq(head_id))
        .execute(conn)
        .expect("Failed to update branch head_id");

    BranchIds { branch_id, head_id }
}

/// Create a version for testing.
pub fn create_version(
    conn: &mut SqliteConnection,
    project_id: i32,
    version_uuid: &str,
    version_number: i32,
    hash: Option<&str>,
) -> i32 {
    let values = if let Some(h) = hash {
        diesel::insert_into(schema::version::table)
            .values((
                schema::version::uuid.eq(version_uuid),
                schema::version::project_id.eq(project_id),
                schema::version::number.eq(version_number),
                schema::version::hash.eq(h),
            ))
            .execute(conn)
    } else {
        diesel::insert_into(schema::version::table)
            .values((
                schema::version::uuid.eq(version_uuid),
                schema::version::project_id.eq(project_id),
                schema::version::number.eq(version_number),
            ))
            .execute(conn)
    };
    values.expect("Failed to insert version");

    schema::version::table
        .filter(schema::version::uuid.eq(version_uuid))
        .select(schema::version::id)
        .first(conn)
        .expect("Failed to get version id")
}

/// Link a version to a head.
pub fn create_head_version(conn: &mut SqliteConnection, head_id: i32, version_id: i32) -> i32 {
    diesel::insert_into(schema::head_version::table)
        .values((
            schema::head_version::head_id.eq(head_id),
            schema::head_version::version_id.eq(version_id),
        ))
        .execute(conn)
        .expect("Failed to insert head_version");

    schema::head_version::table
        .filter(schema::head_version::head_id.eq(head_id))
        .filter(schema::head_version::version_id.eq(version_id))
        .select(schema::head_version::id)
        .first(conn)
        .expect("Failed to get head_version id")
}

/// Create a testbed for testing.
pub fn create_testbed(
    conn: &mut SqliteConnection,
    project_id: i32,
    testbed_uuid: &str,
    testbed_name: &str,
    testbed_slug: &str,
) -> i32 {
    diesel::insert_into(schema::testbed::table)
        .values((
            schema::testbed::uuid.eq(testbed_uuid),
            schema::testbed::project_id.eq(project_id),
            schema::testbed::name.eq(testbed_name),
            schema::testbed::slug.eq(testbed_slug),
            schema::testbed::created.eq(0i64),
            schema::testbed::modified.eq(0i64),
        ))
        .execute(conn)
        .expect("Failed to insert testbed");

    schema::testbed::table
        .filter(schema::testbed::uuid.eq(testbed_uuid))
        .select(schema::testbed::id)
        .first(conn)
        .expect("Failed to get testbed id")
}

/// Create a measure for testing.
pub fn create_measure(
    conn: &mut SqliteConnection,
    project_id: i32,
    measure_uuid: &str,
    measure_name: &str,
    measure_slug: &str,
) -> i32 {
    diesel::insert_into(schema::measure::table)
        .values((
            schema::measure::uuid.eq(measure_uuid),
            schema::measure::project_id.eq(project_id),
            schema::measure::name.eq(measure_name),
            schema::measure::slug.eq(measure_slug),
            schema::measure::units.eq("ns"),
            schema::measure::created.eq(0i64),
            schema::measure::modified.eq(0i64),
        ))
        .execute(conn)
        .expect("Failed to insert measure");

    schema::measure::table
        .filter(schema::measure::uuid.eq(measure_uuid))
        .select(schema::measure::id)
        .first(conn)
        .expect("Failed to get measure id")
}

/// Create a threshold for testing.
pub fn create_threshold(
    conn: &mut SqliteConnection,
    project_id: i32,
    branch_id: i32,
    testbed_id: i32,
    measure_id: i32,
    threshold_uuid: &str,
) -> i32 {
    diesel::insert_into(schema::threshold::table)
        .values((
            schema::threshold::uuid.eq(threshold_uuid),
            schema::threshold::project_id.eq(project_id),
            schema::threshold::branch_id.eq(branch_id),
            schema::threshold::testbed_id.eq(testbed_id),
            schema::threshold::measure_id.eq(measure_id),
            schema::threshold::created.eq(0i64),
            schema::threshold::modified.eq(0i64),
        ))
        .execute(conn)
        .expect("Failed to insert threshold");

    schema::threshold::table
        .filter(schema::threshold::uuid.eq(threshold_uuid))
        .select(schema::threshold::id)
        .first(conn)
        .expect("Failed to get threshold id")
}

/// Create a model for a threshold.
pub fn create_model(
    conn: &mut SqliteConnection,
    threshold_id: i32,
    model_uuid: &str,
    test_type: i32,
) -> i32 {
    diesel::insert_into(schema::model::table)
        .values((
            schema::model::uuid.eq(model_uuid),
            schema::model::threshold_id.eq(threshold_id),
            schema::model::test.eq(test_type),
            schema::model::created.eq(0i64),
        ))
        .execute(conn)
        .expect("Failed to insert model");

    let model_id: i32 = schema::model::table
        .filter(schema::model::uuid.eq(model_uuid))
        .select(schema::model::id)
        .first(conn)
        .expect("Failed to get model id");

    // Update threshold to reference model
    diesel::update(schema::threshold::table.filter(schema::threshold::id.eq(threshold_id)))
        .set(schema::threshold::model_id.eq(model_id))
        .execute(conn)
        .expect("Failed to update threshold with model_id");

    model_id
}

/// Get all `head_version` records for a given head.
pub fn get_head_versions(conn: &mut SqliteConnection, head_id: i32) -> Vec<(i32, i32)> {
    use diesel::QueryDsl as _;

    schema::head_version::table
        .filter(schema::head_version::head_id.eq(head_id))
        .select((
            schema::head_version::head_id,
            schema::head_version::version_id,
        ))
        .load::<(i32, i32)>(conn)
        .expect("Failed to get head_versions")
}

/// Get count of `head_version` records for a given head.
pub fn count_head_versions(conn: &mut SqliteConnection, head_id: i32) -> i64 {
    schema::head_version::table
        .filter(schema::head_version::head_id.eq(head_id))
        .count()
        .get_result(conn)
        .expect("Failed to count head_versions")
}

/// Get all thresholds for a given branch.
pub fn get_thresholds_for_branch(conn: &mut SqliteConnection, branch_id: i32) -> Vec<i32> {
    schema::threshold::table
        .filter(schema::threshold::branch_id.eq(branch_id))
        .select(schema::threshold::id)
        .load::<i32>(conn)
        .expect("Failed to get thresholds")
}

/// Get threshold `model_id`.
pub fn get_threshold_model_id(conn: &mut SqliteConnection, threshold_id: i32) -> Option<i32> {
    schema::threshold::table
        .filter(schema::threshold::id.eq(threshold_id))
        .select(schema::threshold::model_id)
        .first::<Option<i32>>(conn)
        .expect("Failed to get threshold model_id")
}
