use std::str::FromStr;

use diesel::{
    Insertable,
    Queryable,
    SqliteConnection,
};
use dropshot::HttpError;
use schemars::JsonSchema;
use serde::{
    Deserialize,
    Serialize,
};
use uuid::Uuid;

use crate::{
    db::{
        schema,
        schema::perf as perf_table,
    },
    diesel::{
        ExpressionMethods,
        QueryDsl,
        RunQueryDsl,
    },
    util::http_error,
};

mod latency;
mod min_max_avg;
mod throughput;

pub use latency::InsertLatency;
pub use min_max_avg::InsertMinMaxAvg;
pub use throughput::InsertThroughput;

const PERF_ERROR: &str = "Failed to get perf.";

#[derive(Queryable, Debug, Deserialize, Serialize, JsonSchema)]
pub struct QueryPerf {
    pub id: i32,
    pub uuid: String,
    pub report_id: i32,
    pub benchmark_id: i32,
    pub latency_id: i32,
    pub throughput_id: i32,
    pub compute_id: i32,
    pub memory_id: i32,
    pub storage_id: i32,
}

impl QueryPerf {
    pub fn get_id(conn: &SqliteConnection, uuid: &Uuid) -> Result<i32, HttpError> {
        schema::perf::table
            .filter(schema::perf::uuid.eq(uuid.to_string()))
            .select(schema::perf::id)
            .first(conn)
            .map_err(|_| http_error!(PERF_ERROR))
    }

    pub fn get_uuid(conn: &SqliteConnection, id: i32) -> Result<Uuid, HttpError> {
        let uuid: String = schema::perf::table
            .filter(schema::perf::id.eq(id))
            .select(schema::perf::uuid)
            .first(conn)
            .map_err(|_| http_error!(PERF_ERROR))?;
        Uuid::from_str(&uuid).map_err(|_| http_error!(PERF_ERROR))
    }
}

#[derive(Insertable)]
#[table_name = "perf_table"]
pub struct InsertPerf {
    pub uuid:          String,
    pub report_id:     i32,
    pub benchmark_id:  i32,
    pub latency_id:    Option<i32>,
    pub throughput_id: Option<i32>,
    pub compute_id:    Option<i32>,
    pub memory_id:     Option<i32>,
    pub storage_id:    Option<i32>,
}
