use std::str::FromStr;

use diesel::{Insertable, Queryable, SqliteConnection};
use dropshot::HttpError;
use uuid::Uuid;

use crate::{
    diesel::{ExpressionMethods, QueryDsl, RunQueryDsl},
    schema,
    schema::perf as perf_table,
    util::map_http_error,
};

pub mod metric;

#[derive(Queryable)]
pub struct QueryPerf {
    pub id: i32,
    pub uuid: String,
    pub report_id: i32,
    pub iteration: i32,
    pub benchmark_id: i32,
}

impl QueryPerf {
    pub fn get_id(conn: &mut SqliteConnection, uuid: impl ToString) -> Result<i32, HttpError> {
        schema::perf::table
            .filter(schema::perf::uuid.eq(uuid.to_string()))
            .select(schema::perf::id)
            .first(conn)
            .map_err(map_http_error!("Failed to get perf."))
    }

    pub fn get_uuid(conn: &mut SqliteConnection, id: i32) -> Result<Uuid, HttpError> {
        let uuid: String = schema::perf::table
            .filter(schema::perf::id.eq(id))
            .select(schema::perf::uuid)
            .first(conn)
            .map_err(map_http_error!("Failed to get perf."))?;
        Uuid::from_str(&uuid).map_err(map_http_error!("Failed to get perf."))
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
    pub fn from_json(
        conn: &mut SqliteConnection,
        report_id: i32,
        iteration: i32,
        benchmark_id: i32,
    ) -> Result<Self, HttpError> {
        Ok(InsertPerf {
            uuid: Uuid::new_v4().to_string(),
            report_id,
            iteration,
            benchmark_id,
        })
    }
}
