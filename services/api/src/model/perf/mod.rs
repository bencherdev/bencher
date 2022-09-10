use std::str::FromStr;

use bencher_json::report::new::JsonMetrics;
use diesel::{Insertable, Queryable, SqliteConnection};
use dropshot::HttpError;
use uuid::Uuid;

use crate::{
    diesel::{ExpressionMethods, QueryDsl, RunQueryDsl},
    schema,
    schema::perf as perf_table,
    util::http_error,
};

pub mod latency;
pub mod resource;
pub mod throughput;

pub use latency::InsertLatency;
pub use resource::InsertResource;
pub use throughput::InsertThroughput;


#[derive(Queryable)]
pub struct QueryPerf {
    pub id: i32,
    pub uuid: String,
    pub report_id: i32,
    pub iteration: i32,
    pub benchmark_id: i32,
    pub latency_id: Option<i32>,
    pub throughput_id: Option<i32>,
    pub compute_id: Option<i32>,
    pub memory_id: Option<i32>,
    pub storage_id: Option<i32>,
}

impl QueryPerf {
    pub fn get_id(conn: &mut SqliteConnection, uuid: impl ToString) -> Result<i32, HttpError> {
        schema::perf::table
            .filter(schema::perf::uuid.eq(uuid.to_string()))
            .select(schema::perf::id)
            .first(conn)
            .map_err(|_| http_error!("Failed to get perf."))
    }

    pub fn get_uuid(conn: &mut SqliteConnection, id: i32) -> Result<Uuid, HttpError> {
        let uuid: String = schema::perf::table
            .filter(schema::perf::id.eq(id))
            .select(schema::perf::uuid)
            .first(conn)
            .map_err(|_| http_error!("Failed to get perf."))?;
        Uuid::from_str(&uuid).map_err(|_| http_error!("Failed to get perf."))
    }
}

#[derive(Insertable)]
#[diesel(table_name = perf_table)]
pub struct InsertPerf {
    pub uuid: String,
    pub report_id: i32,
    pub iteration: i32,
    pub benchmark_id: i32,
    pub latency_id: Option<i32>,
    pub throughput_id: Option<i32>,
    pub compute_id: Option<i32>,
    pub memory_id: Option<i32>,
    pub storage_id: Option<i32>,
}

impl InsertPerf {
    pub fn from_json(
        conn: &mut SqliteConnection,
        report_id: i32,
        iteration: i32,
        benchmark_id: i32,
        metrics: JsonMetrics,
    ) -> Result<Self, HttpError> {
        Ok(InsertPerf {
            uuid: Uuid::new_v4().to_string(),
            report_id,
            iteration,
            benchmark_id,
            latency_id: InsertLatency::map_json(conn, metrics.latency)?,
            throughput_id: InsertThroughput::map_json(conn, metrics.throughput)?,
            compute_id: InsertResource::map_json(conn, metrics.compute)?,
            memory_id: InsertResource::map_json(conn, metrics.memory)?,
            storage_id: InsertResource::map_json(conn, metrics.storage)?,
        })
    }
}
