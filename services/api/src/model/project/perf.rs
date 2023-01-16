use std::str::FromStr;

use diesel::{Insertable, Queryable, SqliteConnection};
use uuid::Uuid;

use crate::{
    diesel::{ExpressionMethods, QueryDsl, RunQueryDsl},
    error::api_error,
    schema,
    schema::perf as perf_table,
    util::query::fn_get_id,
    ApiError,
};

#[derive(Queryable)]
pub struct QueryPerf {
    pub id: i32,
    pub uuid: String,
    pub report_id: i32,
    pub iteration: i32,
    pub benchmark_id: i32,
}

impl QueryPerf {
    fn_get_id!(perf);

    pub fn get_uuid(conn: &mut SqliteConnection, id: i32) -> Result<Uuid, ApiError> {
        let uuid: String = schema::perf::table
            .filter(schema::perf::id.eq(id))
            .select(schema::perf::uuid)
            .first(conn)
            .map_err(api_error!())?;
        Uuid::from_str(&uuid).map_err(api_error!())
    }
}

#[derive(Insertable)]
#[diesel(table_name = perf_table)]
pub struct InsertPerf {
    pub uuid: String,
    pub report_id: i32,
    pub iteration: i32,
    pub benchmark_id: i32,
}

impl InsertPerf {
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    pub fn from_json(report_id: i32, iteration: usize, benchmark_id: i32) -> Self {
        InsertPerf {
            uuid: Uuid::new_v4().to_string(),
            report_id,
            iteration: iteration as i32,
            benchmark_id,
        }
    }
}
