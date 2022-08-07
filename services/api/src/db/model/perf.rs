use std::str::FromStr;

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
        schema::perf as perf_table,
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
    pub kind: u8,
    // latency
    pub duration: i32,
    pub lower_variance: i32,
    pub upper_variance: i32,
    // throughput
    pub lower_events: i32,
    pub upper_events: i32,
    pub unit_time: i32,
    // compute
    pub min_compute: i32,
    pub max_compute: i32,
    pub avg_compute: i32,
    // memory
    pub min_memory: i32,
    pub min_memory: i32,
    pub avg_memory: i32,
    // storage
    pub min_storage: i32,
    pub max_storage: i32,
    pub avg_storage: i32,
}

impl QueryPerf {
    pub fn get_id(conn: &SqliteConnection, uuid: &Uuid) -> Result<i32, HttpError> {
        schema::perf::table
            .filter(schema::perf::uuid.eq(uuid.to_string()))
            .select(schema::perf::id)
            .first(conn)
            .map_err(|_| http_error!(PERF_ERROR))
    }

    pub fn get_id_from_name(
        conn: &SqliteConnection,
        project_id: i32,
        name: &str,
    ) -> Result<i32, HttpError> {
        schema::perf::table
            .filter(
                schema::perf::project_id
                    .eq(project_id)
                    .and(schema::perf::name.eq(name)),
            )
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
    pub uuid:       String,
    pub project_id: i32,
    pub name:       String,
}

impl InsertPerf {
    pub fn new(project_id: i32, name: String) -> Self {
        Self {
            uuid: Uuid::new_v4().to_string(),
            project_id,
            name,
        }
    }
}
