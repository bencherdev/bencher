#![expect(unused_crate_dependencies)]
#![expect(clippy::expect_used)]

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use diesel::{
    Connection as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _,
    SelectableHelper as _, SqliteConnection,
};

use bencher_schema::{run_migrations, schema};

/// Create an in-memory `SQLite` database with migrations applied
fn setup_database() -> SqliteConnection {
    let mut conn =
        SqliteConnection::establish(":memory:").expect("Failed to create in-memory database");
    run_migrations(&mut conn).expect("Failed to run migrations");
    conn
}

/// Create base entities (organization, project, branch, head) required for all benchmarks
fn setup_base_entities(conn: &mut SqliteConnection) {
    use schema::{branch, head, organization, project};

    diesel::insert_into(organization::table)
        .values((
            organization::uuid.eq("00000000-0000-0000-0000-000000000001"),
            organization::name.eq("Test Org"),
            organization::slug.eq("test-org"),
            organization::created.eq(0i64),
            organization::modified.eq(0i64),
        ))
        .execute(conn)
        .expect("Failed to insert organization");

    diesel::insert_into(project::table)
        .values((
            project::uuid.eq("00000000-0000-0000-0000-000000000002"),
            project::organization_id.eq(1),
            project::name.eq("Test Project"),
            project::slug.eq("test-project"),
            project::visibility.eq(0),
            project::created.eq(0i64),
            project::modified.eq(0i64),
        ))
        .execute(conn)
        .expect("Failed to insert project");

    diesel::insert_into(branch::table)
        .values((
            branch::uuid.eq("00000000-0000-0000-0000-000000000003"),
            branch::project_id.eq(1),
            branch::name.eq("main"),
            branch::slug.eq("main"),
            branch::created.eq(0i64),
            branch::modified.eq(0i64),
        ))
        .execute(conn)
        .expect("Failed to insert branch");

    diesel::insert_into(head::table)
        .values((
            head::uuid.eq("00000000-0000-0000-0000-000000000004"),
            head::branch_id.eq(1),
            head::created.eq(0i64),
        ))
        .execute(conn)
        .expect("Failed to insert head");
}

/// Insert test data for `head_version` benchmarks
fn setup_head_version_data(conn: &mut SqliteConnection, num_versions: usize) -> (i32, Vec<i32>) {
    use schema::version;

    setup_base_entities(conn);

    let mut version_ids = Vec::with_capacity(num_versions);
    for i in 0..num_versions {
        diesel::insert_into(version::table)
            .values((
                version::uuid.eq(format!("00000000-0000-0000-0000-{i:012}")),
                version::project_id.eq(1),
                version::number.eq(i32::try_from(i).expect("version number fits in i32")),
            ))
            .execute(conn)
            .expect("Failed to insert version");
        version_ids.push(i32::try_from(i + 1).expect("version id fits in i32"));
    }

    (1, version_ids) // head_id, version_ids
}

/// Benchmark batch insert for `head_version` (the optimized approach)
fn bench_head_version_batch_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("head_version_insert");

    for num_versions in [10, 50, 100, 255] {
        group.bench_with_input(
            BenchmarkId::new("batch", num_versions),
            &num_versions,
            |b, &num_versions| {
                b.iter_batched(
                    || {
                        let mut conn = setup_database();
                        let (head_id, version_ids) =
                            setup_head_version_data(&mut conn, num_versions);
                        (conn, head_id, version_ids)
                    },
                    |(mut conn, head_id, version_ids)| {
                        use schema::head_version;

                        let inserts: Vec<_> = version_ids
                            .iter()
                            .map(|&version_id| {
                                (
                                    head_version::head_id.eq(head_id),
                                    head_version::version_id.eq(version_id),
                                )
                            })
                            .collect();

                        diesel::insert_into(head_version::table)
                            .values(&inserts)
                            .execute(&mut conn)
                            .expect("Failed to batch insert head_version");
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

/// Setup test data for threshold benchmarks
fn setup_threshold_data(conn: &mut SqliteConnection, num_thresholds: usize) -> i32 {
    use schema::{measure, model, testbed, threshold};

    setup_base_entities(conn);

    for i in 0..num_thresholds {
        diesel::insert_into(testbed::table)
            .values((
                testbed::uuid.eq(format!("10000000-0000-0000-0000-{i:012}")),
                testbed::project_id.eq(1),
                testbed::name.eq(format!("testbed-{i}")),
                testbed::slug.eq(format!("testbed-{i}")),
                testbed::created.eq(0i64),
                testbed::modified.eq(0i64),
            ))
            .execute(conn)
            .expect("Failed to insert testbed");

        diesel::insert_into(measure::table)
            .values((
                measure::uuid.eq(format!("20000000-0000-0000-0000-{i:012}")),
                measure::project_id.eq(1),
                measure::name.eq(format!("measure-{i}")),
                measure::slug.eq(format!("measure-{i}")),
                measure::units.eq("ns"),
                measure::created.eq(0i64),
                measure::modified.eq(0i64),
            ))
            .execute(conn)
            .expect("Failed to insert measure");

        diesel::insert_into(threshold::table)
            .values((
                threshold::uuid.eq(format!("30000000-0000-0000-0000-{i:012}")),
                threshold::project_id.eq(1),
                threshold::branch_id.eq(1),
                threshold::testbed_id.eq(i32::try_from(i + 1).expect("testbed id fits in i32")),
                threshold::measure_id.eq(i32::try_from(i + 1).expect("measure id fits in i32")),
                threshold::created.eq(0i64),
                threshold::modified.eq(0i64),
            ))
            .execute(conn)
            .expect("Failed to insert threshold");

        diesel::insert_into(model::table)
            .values((
                model::uuid.eq(format!("40000000-0000-0000-0000-{i:012}")),
                model::threshold_id.eq(i32::try_from(i + 1).expect("threshold id fits in i32")),
                model::test.eq(0),
                model::created.eq(0i64),
            ))
            .execute(conn)
            .expect("Failed to insert model");

        diesel::update(
            threshold::table
                .filter(threshold::id.eq(i32::try_from(i + 1).expect("threshold id fits in i32"))),
        )
        .set(threshold::model_id.eq(i32::try_from(i + 1).expect("model id fits in i32")))
        .execute(conn)
        .expect("Failed to update threshold with model_id");
    }

    1 // branch_id
}

/// Benchmark JOIN query for thresholds with models (the optimized approach)
fn bench_threshold_join_query(c: &mut Criterion) {
    use bencher_schema::model::project::threshold::{QueryThreshold, model::QueryModel};

    let mut group = c.benchmark_group("threshold_query");

    for num_thresholds in [5, 10, 20, 50] {
        group.bench_with_input(
            BenchmarkId::new("join", num_thresholds),
            &num_thresholds,
            |b, &num_thresholds| {
                b.iter_batched(
                    || {
                        let mut conn = setup_database();
                        let branch_id = setup_threshold_data(&mut conn, num_thresholds);
                        (conn, branch_id)
                    },
                    |(mut conn, branch_id)| {
                        use diesel::JoinOnDsl as _;
                        use diesel::NullableExpressionMethods as _;

                        let _results: Vec<(QueryThreshold, Option<QueryModel>)> =
                            schema::threshold::table
                                .left_join(schema::model::table.on(
                                    schema::model::id.nullable().eq(schema::threshold::model_id),
                                ))
                                .filter(schema::threshold::branch_id.eq(branch_id))
                                .select((
                                    QueryThreshold::as_select(),
                                    Option::<QueryModel>::as_select(),
                                ))
                                .load(&mut conn)
                                .expect("Failed to query thresholds with models");
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_head_version_batch_insert,
    bench_threshold_join_query
);
criterion_main!(benches);
