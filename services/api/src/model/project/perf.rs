use std::str::FromStr;

use diesel::{Insertable, Queryable};
use uuid::Uuid;

use crate::{
    context::DbConnection,
    diesel::{ExpressionMethods, QueryDsl, RunQueryDsl},
    schema,
    schema::perf as perf_table,
    util::query::{fn_get, fn_get_id},
    ApiError,
};

use super::{
    benchmark::BenchmarkId,
    report::{QueryReport, ReportId},
};

crate::util::typed_id::typed_id!(PerfId);

#[derive(Queryable, Identifiable, Associations)]
#[diesel(table_name = perf_table)]
#[diesel(belongs_to(QueryReport, foreign_key = report_id))]
pub struct QueryPerf {
    pub id: PerfId,
    pub uuid: String,
    pub report_id: ReportId,
    pub iteration: i32,
    pub benchmark_id: BenchmarkId,
}

impl QueryPerf {
    fn_get!(perf);
    fn_get_id!(perf, PerfId);

    pub fn get_uuid(conn: &mut DbConnection, id: PerfId) -> Result<Uuid, ApiError> {
        let uuid: String = schema::perf::table
            .filter(schema::perf::id.eq(id))
            .select(schema::perf::uuid)
            .first(conn)
            .map_err(ApiError::from)?;
        Uuid::from_str(&uuid).map_err(ApiError::from)
    }
}

#[derive(Insertable)]
#[diesel(table_name = perf_table)]
pub struct InsertPerf {
    pub uuid: String,
    pub report_id: ReportId,
    pub iteration: i32,
    pub benchmark_id: BenchmarkId,
}

impl InsertPerf {
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    pub fn from_json(report_id: ReportId, iteration: usize, benchmark_id: BenchmarkId) -> Self {
        InsertPerf {
            uuid: Uuid::new_v4().to_string(),
            report_id,
            iteration: iteration as i32,
            benchmark_id,
        }
    }
}
