use bencher_json::{project::perf::Iteration, PerfUuid};

use crate::{
    schema::perf as perf_table,
    util::fn_get::{fn_get, fn_get_id, fn_get_uuid},
};

use super::{
    benchmark::BenchmarkId,
    report::{QueryReport, ReportId},
};

crate::util::typed_id::typed_id!(PerfId);

#[derive(diesel::Queryable, diesel::Identifiable, diesel::Associations)]
#[diesel(table_name = perf_table)]
#[diesel(belongs_to(QueryReport, foreign_key = report_id))]
pub struct QueryPerf {
    pub id: PerfId,
    pub uuid: PerfUuid,
    pub report_id: ReportId,
    pub iteration: Iteration,
    pub benchmark_id: BenchmarkId,
}

impl QueryPerf {
    fn_get!(perf, PerfId);
    fn_get_id!(perf, PerfId, PerfUuid);
    fn_get_uuid!(perf, PerfId, PerfUuid);
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = perf_table)]
pub struct InsertPerf {
    pub uuid: PerfUuid,
    pub report_id: ReportId,
    pub iteration: Iteration,
    pub benchmark_id: BenchmarkId,
}

impl InsertPerf {
    pub fn from_json(report_id: ReportId, iteration: Iteration, benchmark_id: BenchmarkId) -> Self {
        InsertPerf {
            uuid: PerfUuid::new(),
            report_id,
            iteration,
            benchmark_id,
        }
    }
}
