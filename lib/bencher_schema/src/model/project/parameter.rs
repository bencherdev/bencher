use bencher_json::{DateTime, JsonParameters, ParameterUuid};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;

use crate::{
    context::DbConnection,
    error::issue_error,
    macros::fn_get::{fn_from_uuid, fn_get, fn_get_id, fn_get_uuid},
    schema::{self, parameter as parameter_table},
};

use super::benchmark::{BenchmarkId, QueryBenchmark};

crate::macros::typed_id::typed_id!(ParameterId);

/// A parameter set: one grid point under its benchmark.
///
/// Parameter sets have neither a name nor a slug, so they are UUID addressed only,
/// following the `report` and `alert` precedent.
#[derive(
    Debug, Clone, diesel::Queryable, diesel::Identifiable, diesel::Associations, diesel::Selectable,
)]
#[diesel(table_name = parameter_table)]
#[diesel(belongs_to(QueryBenchmark, foreign_key = benchmark_id))]
pub struct QueryParameter {
    pub id: ParameterId,
    pub uuid: ParameterUuid,
    pub benchmark_id: BenchmarkId,
    pub parameters: JsonParameters,
    pub created: DateTime,
    pub modified: DateTime,
    pub archived: Option<DateTime>,
}

impl QueryParameter {
    fn_get!(parameter, ParameterId);
    fn_get_id!(parameter, ParameterId, ParameterUuid);
    fn_get_uuid!(parameter, ParameterId, ParameterUuid);
    fn_from_uuid!(
        benchmark_id,
        BenchmarkId,
        parameter,
        ParameterUuid,
        Parameter
    );

    /// Get the benchmark's empty parameter set.
    ///
    /// Every benchmark is created atomically with its empty parameter set,
    /// so a missing row is data corruption and not a missing get-or-create.
    pub fn get_empty_set_id(
        conn: &mut DbConnection,
        benchmark_id: BenchmarkId,
    ) -> Result<ParameterId, HttpError> {
        schema::parameter::table
            .filter(schema::parameter::benchmark_id.eq(benchmark_id))
            .filter(schema::parameter::parameters.eq(JsonParameters::default()))
            .select(schema::parameter::id)
            .first(conn)
            .map_err(|e| {
                let message = format!(
                    "Failed to query the empty parameter set for benchmark ({benchmark_id})"
                );
                issue_error(&message, &message, e)
            })
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = parameter_table)]
pub struct InsertParameter {
    pub uuid: ParameterUuid,
    pub benchmark_id: BenchmarkId,
    pub parameters: JsonParameters,
    pub created: DateTime,
    pub modified: DateTime,
    pub archived: Option<DateTime>,
}

impl InsertParameter {
    /// The empty parameter set that every benchmark is born with.
    ///
    /// The timestamp is the benchmark's own creation timestamp:
    /// the parameter set is created in the same transaction as its benchmark.
    pub fn empty_set(benchmark_id: BenchmarkId, timestamp: DateTime) -> Self {
        Self {
            uuid: ParameterUuid::new(),
            benchmark_id,
            parameters: JsonParameters::default(),
            created: timestamp,
            modified: timestamp,
            archived: None,
        }
    }

    pub fn into_query(self, id: ParameterId) -> QueryParameter {
        let Self {
            uuid,
            benchmark_id,
            parameters,
            created,
            modified,
            archived,
        } = self;
        QueryParameter {
            id,
            uuid,
            benchmark_id,
            parameters,
            created,
            modified,
            archived,
        }
    }
}

#[derive(Debug, Clone, diesel::AsChangeset)]
#[diesel(table_name = parameter_table)]
pub struct UpdateParameter {
    pub parameters: Option<JsonParameters>,
    pub modified: DateTime,
    pub archived: Option<Option<DateTime>>,
}

#[cfg(test)]
mod tests {
    use bencher_json::{DateTime, JsonParameters, ParameterUuid};
    use diesel::{
        ExpressionMethods as _, QueryDsl as _, QueryResult, RunQueryDsl as _, SqliteConnection,
        connection::SimpleConnection as _,
    };
    use diesel_migrations::MigrationHarness as _;

    use crate::{
        model::project::benchmark::BenchmarkId,
        schema,
        test_util::{create_base_entities, create_benchmark, setup_test_db},
    };

    fn parameters(parameters: &str) -> JsonParameters {
        parameters.parse().expect("Failed to parse parameters")
    }

    fn insert_parameter(
        conn: &mut SqliteConnection,
        benchmark_id: BenchmarkId,
        parameters: &JsonParameters,
    ) -> QueryResult<usize> {
        diesel::insert_into(schema::parameter::table)
            .values((
                schema::parameter::uuid.eq(ParameterUuid::new()),
                schema::parameter::benchmark_id.eq(benchmark_id),
                schema::parameter::parameters.eq(parameters),
                schema::parameter::created.eq(DateTime::TEST),
                schema::parameter::modified.eq(DateTime::TEST),
            ))
            .execute(conn)
    }

    fn count_parameters(conn: &mut SqliteConnection, benchmark_id: BenchmarkId) -> i64 {
        schema::parameter::table
            .filter(schema::parameter::benchmark_id.eq(benchmark_id))
            .count()
            .get_result(conn)
            .expect("Failed to count parameters")
    }

    #[test]
    fn key_order_collides_on_unique() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let benchmark_id = create_benchmark(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "bench1",
            "bench1",
        );

        insert_parameter(&mut conn, benchmark_id, &parameters(r#"{"b": 1, "a": 2}"#))
            .expect("Failed to insert parameter");
        let collision =
            insert_parameter(&mut conn, benchmark_id, &parameters(r#"{"a": 2, "b": 1}"#));

        assert!(
            collision.is_err(),
            "logically equal parameter sets must collide on UNIQUE(benchmark_id, parameters)"
        );
        // The empty set the benchmark was born with, plus the one that landed.
        assert_eq!(count_parameters(&mut conn, benchmark_id), 2);
    }

    #[test]
    fn number_spelling_collides_on_unique() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let benchmark_id = create_benchmark(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "bench1",
            "bench1",
        );

        insert_parameter(&mut conn, benchmark_id, &parameters(r#"{"n": 16}"#))
            .expect("Failed to insert parameter");
        for spelling in [r#"{"n": 16.0}"#, r#"{"n": 1.6e1}"#] {
            assert!(
                insert_parameter(&mut conn, benchmark_id, &parameters(spelling)).is_err(),
                "{spelling} must collide with 16"
            );
        }

        assert_eq!(count_parameters(&mut conn, benchmark_id), 2);
    }

    #[test]
    fn identical_parameters_under_distinct_benchmarks() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let first = create_benchmark(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "bench1",
            "bench1",
        );
        let second = create_benchmark(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000011",
            "bench2",
            "bench2",
        );

        let grid_point = parameters(r#"{"size_mb": 16}"#);
        insert_parameter(&mut conn, first, &grid_point).expect("Failed to insert parameter");
        insert_parameter(&mut conn, second, &grid_point).expect("Failed to insert parameter");

        assert_eq!(count_parameters(&mut conn, first), 2);
        assert_eq!(count_parameters(&mut conn, second), 2);
    }

    /// Seed the pre-migration shape: benchmarks and `report_benchmark` rows with
    /// no `parameter_id`, written as raw SQL because the Diesel DSL describes the
    /// post-migration schema.
    fn seed_legacy_rows(conn: &mut SqliteConnection) {
        conn.batch_execute(
            "INSERT INTO organization (uuid, name, slug, created, modified)
                VALUES ('00000000-0000-0000-0000-000000000001', 'Org', 'org', 0, 0);
            INSERT INTO project (uuid, organization_id, name, slug, visibility, created, modified)
                VALUES ('00000000-0000-0000-0000-000000000002', 1, 'Project', 'project', 0, 0, 0);
            INSERT INTO branch (uuid, project_id, name, slug, created, modified)
                VALUES ('00000000-0000-0000-0000-000000000003', 1, 'main', 'main', 0, 0);
            INSERT INTO head (uuid, branch_id, created)
                VALUES ('00000000-0000-0000-0000-000000000004', 1, 0);
            UPDATE branch SET head_id = 1 WHERE id = 1;
            INSERT INTO version (uuid, project_id, number)
                VALUES ('00000000-0000-0000-0000-000000000005', 1, 1);
            INSERT INTO head_version (head_id, version_id) VALUES (1, 1);
            INSERT INTO testbed (uuid, project_id, name, slug, created, modified)
                VALUES ('00000000-0000-0000-0000-000000000006', 1, 'localhost', 'localhost', 0, 0);
            INSERT INTO report (uuid, project_id, head_id, version_id, testbed_id, adapter, start_time, end_time, created)
                VALUES ('00000000-0000-0000-0000-000000000007', 1, 1, 1, 1, 0, 0, 0, 0);
            INSERT INTO benchmark (uuid, project_id, name, slug, created, modified)
                VALUES ('00000000-0000-0000-0000-000000000008', 1, 'bench1', 'bench1', 0, 0);
            INSERT INTO benchmark (uuid, project_id, name, slug, created, modified)
                VALUES ('00000000-0000-0000-0000-000000000009', 1, 'bench2', 'bench2', 0, 0);
            INSERT INTO report_benchmark (uuid, report_id, iteration, benchmark_id)
                VALUES ('00000000-0000-0000-0000-000000000010', 1, 0, 1);
            INSERT INTO report_benchmark (uuid, report_id, iteration, benchmark_id)
                VALUES ('00000000-0000-0000-0000-000000000011', 1, 1, 1);
            INSERT INTO report_benchmark (uuid, report_id, iteration, benchmark_id)
                VALUES ('00000000-0000-0000-0000-000000000012', 1, 0, 2);",
        )
        .expect("Failed to seed legacy rows");
    }

    #[test]
    fn migration_backfills_empty_parameter_sets() {
        let mut conn = setup_test_db();

        // Foreign keys cannot be toggled inside a transaction, and Diesel runs each
        // migration in one, so they are disabled around the revert and re-apply.
        conn.batch_execute("PRAGMA foreign_keys = OFF")
            .expect("Failed to disable foreign keys");
        conn.revert_last_migration(crate::MIGRATIONS)
            .expect("Failed to revert the parameter migration");

        seed_legacy_rows(&mut conn);

        conn.run_pending_migrations(crate::MIGRATIONS)
            .expect("Failed to re-apply the parameter migration");
        conn.batch_execute("PRAGMA foreign_keys = ON")
            .expect("Failed to enable foreign keys");

        let benchmark_ids: Vec<BenchmarkId> = schema::benchmark::table
            .order(schema::benchmark::id.asc())
            .select(schema::benchmark::id)
            .load(&mut conn)
            .expect("Failed to load benchmarks");
        assert_eq!(benchmark_ids.len(), 2);

        for benchmark_id in benchmark_ids {
            let backfilled: Vec<JsonParameters> = schema::parameter::table
                .filter(schema::parameter::benchmark_id.eq(benchmark_id))
                .select(schema::parameter::parameters)
                .load(&mut conn)
                .expect("Failed to load parameters");
            assert_eq!(
                backfilled,
                vec![JsonParameters::default()],
                "every benchmark gets exactly one empty parameter set"
            );

            // The migration mints the canonical empty object in SQL, so a set minted
            // in Rust has to be byte identical to it.
            assert!(
                insert_parameter(&mut conn, benchmark_id, &JsonParameters::default()).is_err(),
                "the backfilled empty set must collide with a Rust minted one"
            );
        }

        let report_benchmarks: Vec<(BenchmarkId, super::ParameterId)> =
            schema::report_benchmark::table
                .select((
                    schema::report_benchmark::benchmark_id,
                    schema::report_benchmark::parameter_id,
                ))
                .load(&mut conn)
                .expect("Failed to load report benchmarks");
        assert_eq!(report_benchmarks.len(), 3);
        for (benchmark_id, parameter_id) in report_benchmarks {
            let empty_set_id = super::QueryParameter::get_empty_set_id(&mut conn, benchmark_id)
                .expect("Failed to get the empty parameter set");
            assert_eq!(
                parameter_id, empty_set_id,
                "every report benchmark points at its own benchmark's empty set"
            );
        }
    }

    #[test]
    fn migration_down_and_up_is_idempotent() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let benchmark_id = create_benchmark(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "bench1",
            "bench1",
        );
        assert_eq!(count_parameters(&mut conn, benchmark_id), 1);

        conn.batch_execute("PRAGMA foreign_keys = OFF")
            .expect("Failed to disable foreign keys");
        conn.revert_last_migration(crate::MIGRATIONS)
            .expect("Failed to revert the parameter migration");
        conn.run_pending_migrations(crate::MIGRATIONS)
            .expect("Failed to re-apply the parameter migration");
        conn.batch_execute("PRAGMA foreign_keys = ON")
            .expect("Failed to enable foreign keys");

        assert_eq!(count_parameters(&mut conn, benchmark_id), 1);
    }
}
