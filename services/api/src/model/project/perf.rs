use std::str::FromStr;

use diesel::{Insertable, Queryable, SqliteConnection};
use uuid::Uuid;

use crate::{
    diesel::{ExpressionMethods, QueryDsl, RunQueryDsl},
    error::api_error,
    schema,
    schema::perf as perf_table,
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
    pub fn get_id(conn: &mut SqliteConnection, uuid: impl ToString) -> Result<i32, ApiError> {
        schema::perf::table
            .filter(schema::perf::uuid.eq(uuid.to_string()))
            .select(schema::perf::id)
            .first(conn)
            .map_err(api_error!())
    }

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
    pub fn from_json(report_id: i32, iteration: usize, benchmark_id: i32) -> Self {
        InsertPerf {
            uuid: Uuid::new_v4().to_string(),
            report_id,
            iteration: iteration as i32,
            benchmark_id,
        }
    }
}
