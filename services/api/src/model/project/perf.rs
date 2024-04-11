use bencher_json::{project::report::Iteration, ReportBenchmarkUuid};

use crate::{
    model::project::benchmark::BenchmarkId,
    schema::report_benchmark as report_benchmark_table,
    util::fn_get::{fn_get, fn_get_id, fn_get_uuid},
};

use super::{QueryReport, ReportId};

crate::util::typed_id::typed_id!(ReportBenchmarkId);

#[derive(diesel::Queryable, diesel::Identifiable, diesel::Associations)]
#[diesel(table_name = report_benchmark_table)]
#[diesel(belongs_to(QueryReport, foreign_key = report_id))]
pub struct QueryReportBenchmark {
    pub id: ReportBenchmarkId,
    pub uuid: ReportBenchmarkUuid,
    pub report_id: ReportId,
    pub iteration: Iteration,
    pub benchmark_id: BenchmarkId,
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
}

impl InsertReportBenchmark {
    pub fn from_json(report_id: ReportId, iteration: Iteration, benchmark_id: BenchmarkId) -> Self {
        InsertReportBenchmark {
            uuid: ReportBenchmarkUuid::new(),
            report_id,
            iteration,
            benchmark_id,
        }
    }
}
