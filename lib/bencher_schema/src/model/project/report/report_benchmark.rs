use bencher_json::{ReportBenchmarkUuid, project::report::Iteration};

use crate::{
    macros::fn_get::{fn_get, fn_get_id, fn_get_uuid},
    model::project::{benchmark::BenchmarkId, parameter::ParameterId},
    schema::report_benchmark as report_benchmark_table,
};

use super::{QueryReport, ReportId};

crate::macros::typed_id::typed_id!(ReportBenchmarkId);

#[derive(diesel::Queryable, diesel::Identifiable, diesel::Associations)]
#[diesel(table_name = report_benchmark_table)]
#[diesel(belongs_to(QueryReport, foreign_key = report_id))]
pub struct QueryReportBenchmark {
    pub id: ReportBenchmarkId,
    pub uuid: ReportBenchmarkUuid,
    pub report_id: ReportId,
    pub iteration: Iteration,
    pub benchmark_id: BenchmarkId,
    pub parameter_id: ParameterId,
}

impl QueryReportBenchmark {
    fn_get!(report_benchmark, ReportBenchmarkId);
    fn_get_id!(report_benchmark, ReportBenchmarkId, ReportBenchmarkUuid);
    fn_get_uuid!(report_benchmark, ReportBenchmarkId, ReportBenchmarkUuid);
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = report_benchmark_table)]
pub struct InsertReportBenchmark {
    pub uuid: ReportBenchmarkUuid,
    pub report_id: ReportId,
    pub iteration: Iteration,
    pub benchmark_id: BenchmarkId,
    pub parameter_id: ParameterId,
}

impl InsertReportBenchmark {
    pub fn from_json(
        report_id: ReportId,
        iteration: Iteration,
        benchmark_id: BenchmarkId,
        parameter_id: ParameterId,
    ) -> Self {
        InsertReportBenchmark {
            uuid: ReportBenchmarkUuid::new(),
            report_id,
            iteration,
            benchmark_id,
            parameter_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use bencher_json::{DateTime, ParameterSet, ParameterUuid, ReportBenchmarkUuid};
    use diesel::{
        ExpressionMethods as _, QueryDsl as _, QueryResult, RunQueryDsl as _, SqliteConnection,
    };

    use crate::{
        macros::sql::last_insert_rowid,
        model::project::{benchmark::BenchmarkId, parameter::ParameterId, report::ReportId},
        schema,
        test_util::{
            create_base_entities, create_benchmark, create_branch_with_head, create_report,
            create_testbed, create_version, get_empty_parameter, setup_test_db,
        },
    };

    struct TestRows {
        report: ReportId,
        benchmark: BenchmarkId,
        empty_set: ParameterId,
        grid_point: ParameterId,
    }

    fn seed(conn: &mut SqliteConnection) -> TestRows {
        let base = create_base_entities(conn);
        let branch = create_branch_with_head(
            conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000003",
            "main",
            "main",
            "00000000-0000-0000-0000-000000000004",
        );
        let version_id = create_version(
            conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000005",
            1,
            None,
        );
        let testbed_id = create_testbed(
            conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000006",
            "localhost",
            "localhost",
        );
        let report_id = create_report(
            conn,
            "00000000-0000-0000-0000-000000000007",
            base.project_id,
            branch.head_id,
            version_id,
            testbed_id,
        );
        let benchmark_id = create_benchmark(
            conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000008",
            "bench1",
            "bench1",
        );
        let empty_set_id = get_empty_parameter(conn, benchmark_id);

        let grid_point: ParameterSet =
            r#"{"size_mb": 16}"#.parse().expect("Failed to parse parameters");
        diesel::insert_into(schema::parameter::table)
            .values((
                schema::parameter::uuid.eq(ParameterUuid::new()),
                schema::parameter::benchmark_id.eq(benchmark_id),
                schema::parameter::set.eq(&grid_point),
                schema::parameter::created.eq(DateTime::TEST),
                schema::parameter::modified.eq(DateTime::TEST),
            ))
            .execute(&mut *conn)
            .expect("Failed to insert parameter");
        let grid_point_id: ParameterId = diesel::select(last_insert_rowid())
            .get_result(&mut *conn)
            .expect("Failed to get parameter id");

        TestRows {
            report: report_id,
            benchmark: benchmark_id,
            empty_set: empty_set_id,
            grid_point: grid_point_id,
        }
    }

    fn insert_report_benchmark(
        conn: &mut SqliteConnection,
        rows: &TestRows,
        parameter_id: ParameterId,
    ) -> QueryResult<usize> {
        diesel::insert_into(schema::report_benchmark::table)
            .values((
                schema::report_benchmark::uuid.eq(ReportBenchmarkUuid::new()),
                schema::report_benchmark::report_id.eq(rows.report),
                schema::report_benchmark::iteration.eq(0),
                schema::report_benchmark::benchmark_id.eq(rows.benchmark),
                schema::report_benchmark::parameter_id.eq(parameter_id),
            ))
            .execute(conn)
    }

    #[test]
    fn grid_points_coexist_in_one_iteration() {
        let mut conn = setup_test_db();
        let rows = seed(&mut conn);

        insert_report_benchmark(&mut conn, &rows, rows.empty_set)
            .expect("Failed to insert the empty set report benchmark");
        insert_report_benchmark(&mut conn, &rows, rows.grid_point)
            .expect("Failed to insert the grid point report benchmark");

        let count: i64 = schema::report_benchmark::table
            .filter(schema::report_benchmark::report_id.eq(rows.report))
            .count()
            .get_result(&mut conn)
            .expect("Failed to count report benchmarks");
        assert_eq!(count, 2);
    }

    #[test]
    fn same_grid_point_twice_collides() {
        let mut conn = setup_test_db();
        let rows = seed(&mut conn);

        insert_report_benchmark(&mut conn, &rows, rows.grid_point)
            .expect("Failed to insert the grid point report benchmark");
        assert!(
            insert_report_benchmark(&mut conn, &rows, rows.grid_point).is_err(),
            "one parameter set cannot appear twice in one report iteration"
        );
    }
}
