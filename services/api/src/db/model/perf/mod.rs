use std::str::FromStr;

use bencher_json::report::JsonNewPerf;
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

use super::benchmark::{
    InsertBenchmark,
    QueryBenchmark,
};

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

impl InsertPerf {
    pub fn from_json(
        conn: &SqliteConnection,
        project_id: i32,
        report_id: i32,
        benchmark_name: String,
        json_perf: JsonNewPerf,
    ) -> Result<(Uuid, Uuid), HttpError> {
        let benchmark_id =
            if let Ok(id) = QueryBenchmark::get_id_from_name(conn, project_id, &benchmark_name) {
                id
            } else {
                let insert_benchmark = InsertBenchmark::new(project_id, benchmark_name);
                diesel::insert_into(schema::benchmark::table)
                    .values(&insert_benchmark)
                    .execute(conn)
                    .map_err(|_| http_error!("Failed to create benchmark."))?;

                schema::benchmark::table
                    .filter(schema::benchmark::uuid.eq(&insert_benchmark.uuid))
                    .select(schema::benchmark::id)
                    .first::<i32>(conn)
                    .map_err(|_| http_error!("Failed to create benchmark."))?
            };

        let perf_uuid = Uuid::new_v4();
        let insert_perf = InsertPerf {
            uuid: perf_uuid.to_string(),
            report_id,
            benchmark_id,
            latency_id: InsertLatency::map_json(conn, json_perf.latency)?,
            throughput_id: InsertThroughput::map_json(conn, json_perf.throughput)?,
            compute_id: InsertMinMaxAvg::map_json(conn, json_perf.compute)?,
            memory_id: InsertMinMaxAvg::map_json(conn, json_perf.memory)?,
            storage_id: InsertMinMaxAvg::map_json(conn, json_perf.storage)?,
        };
        diesel::insert_into(schema::perf::table)
            .values(&insert_perf)
            .execute(conn)
            .map_err(|_| http_error!("Failed to create benchmark data."))?;

        let benchmark_uuid = QueryBenchmark::get_uuid(conn, benchmark_id)?;

        Ok((benchmark_uuid, perf_uuid))
    }
}
