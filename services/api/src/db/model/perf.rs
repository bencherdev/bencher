use std::{
    ops::BitAnd,
    str::FromStr,
};

use bencher_json::report::{
    JsonLatency,
    JsonMinMaxAvg,
    JsonThroughput,
};
use diesel::{
    expression_methods::BoolExpressionMethods,
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
        schema::{
            latency as latency_table,
            min_max_avg as min_max_avg_table,
            perf as perf_table,
            throughput as throughput_table,
        },
    },
    diesel::{
        ExpressionMethods,
        QueryDsl,
        RunQueryDsl,
    },
    util::http_error,
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

#[derive(Insertable)]
#[table_name = "latency_table"]
pub struct InsertLatency {
    pub uuid:           String,
    pub lower_variance: i64,
    pub upper_variance: i64,
    pub duration:       i64,
}

impl From<JsonLatency> for InsertLatency {
    fn from(latency: JsonLatency) -> Self {
        let JsonLatency {
            lower_variance,
            upper_variance,
            duration,
        } = latency;
        Self {
            uuid:           Uuid::new_v4().to_string(),
            lower_variance: lower_variance.as_nanos() as i64,
            upper_variance: upper_variance.as_nanos() as i64,
            duration:       duration.as_nanos() as i64,
        }
    }
}

impl InsertLatency {
    pub fn map_json(
        conn: &SqliteConnection,
        latency: Option<JsonLatency>,
    ) -> Result<Option<i32>, HttpError> {
        Ok(if let Some(json_latency) = latency {
            let insert_latency: InsertLatency = json_latency.into();
            diesel::insert_into(schema::latency::table)
                .values(&insert_latency)
                .execute(&*conn)
                .map_err(|_| http_error!("Failed to create benchmark data."))?;

            Some(
                schema::latency::table
                    .filter(schema::latency::uuid.eq(&insert_latency.uuid))
                    .select(schema::latency::id)
                    .first::<i32>(&*conn)
                    .map_err(|_| http_error!("Failed to create benchmark data."))?,
            )
        } else {
            None
        })
    }
}

#[derive(Insertable)]
#[table_name = "throughput_table"]
pub struct InsertThroughput {
    pub uuid:         String,
    pub lower_events: f64,
    pub upper_events: f64,
    pub unit_time:    i64,
}

impl From<JsonThroughput> for InsertThroughput {
    fn from(throughput: JsonThroughput) -> Self {
        let JsonThroughput {
            lower_events,
            upper_events,
            unit_time,
        } = throughput;
        Self {
            uuid: Uuid::new_v4().to_string(),
            lower_events,
            upper_events,
            unit_time: unit_time.as_nanos() as i64,
        }
    }
}

impl InsertThroughput {
    pub fn map_json(
        conn: &SqliteConnection,
        throughput: Option<JsonThroughput>,
    ) -> Result<Option<i32>, HttpError> {
        Ok(if let Some(json_throughput) = throughput {
            let insert_throughput: InsertThroughput = json_throughput.into();
            diesel::insert_into(schema::throughput::table)
                .values(&insert_throughput)
                .execute(&*conn)
                .map_err(|_| http_error!("Failed to create benchmark data."))?;

            Some(
                schema::throughput::table
                    .filter(schema::throughput::uuid.eq(&insert_throughput.uuid))
                    .select(schema::throughput::id)
                    .first::<i32>(&*conn)
                    .map_err(|_| http_error!("Failed to create benchmark data."))?,
            )
        } else {
            None
        })
    }
}

#[derive(Insertable)]
#[table_name = "min_max_avg_table"]
pub struct InsertMinMaxAvg {
    pub uuid: String,
    pub min:  f64,
    pub max:  f64,
    pub avg:  f64,
}

impl From<JsonMinMaxAvg> for InsertMinMaxAvg {
    fn from(min_max_avg: JsonMinMaxAvg) -> Self {
        let JsonMinMaxAvg { min, max, avg } = min_max_avg;
        Self {
            uuid: Uuid::new_v4().to_string(),
            min,
            max,
            avg,
        }
    }
}

impl InsertMinMaxAvg {
    pub fn map_json(
        conn: &SqliteConnection,
        min_max_avg: Option<JsonMinMaxAvg>,
    ) -> Result<Option<i32>, HttpError> {
        Ok(if let Some(json_min_max_avg) = min_max_avg {
            let insert_min_max_avg: InsertMinMaxAvg = json_min_max_avg.into();
            diesel::insert_into(schema::min_max_avg::table)
                .values(&insert_min_max_avg)
                .execute(&*conn)
                .map_err(|_| http_error!("Failed to create benchmark data."))?;

            Some(
                schema::min_max_avg::table
                    .filter(schema::min_max_avg::uuid.eq(&insert_min_max_avg.uuid))
                    .select(schema::min_max_avg::id)
                    .first::<i32>(&*conn)
                    .map_err(|_| http_error!("Failed to create benchmark data."))?,
            )
        } else {
            None
        })
    }
}
