use bencher_json::PerfUuid;
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};

use crate::{
    schema,
    schema::perf as perf_table,
    util::query::{fn_get, fn_get_id, fn_get_uuid},
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
    pub iteration: i32,
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
    pub iteration: i32,
    pub benchmark_id: BenchmarkId,
}

impl InsertPerf {
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    pub fn from_json(report_id: ReportId, iteration: usize, benchmark_id: BenchmarkId) -> Self {
        InsertPerf {
            uuid: PerfUuid::new(),
            report_id,
            iteration: iteration as i32,
            benchmark_id,
        }
    }
}
