use std::{
    ops::BitAnd,
    str::FromStr,
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

// #[derive(Debug, Copy, Clone)]
// enum PerfKind {
//     Latency    = 1,
//     Throughput = 2,
//     Compute    = 4,
//     Memory     = 8,
//     Storage    = 16,
// }

// impl PerfKind {
//     fn has_kind(self, kind: u8) -> bool {
//         self & kind == kind
//     }
// }

#[derive(Queryable, Debug, Deserialize, Serialize, JsonSchema)]
pub struct QueryPerf {
    pub id: i32,
    pub uuid: String,
    pub report_id: i32,
    pub benchmark_id: i32,
    pub kind: i32,
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
    pub max_memory: i32,
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
    pub uuid:           String,
    pub report_id:      i32,
    pub benchmark_id:   i32,
    pub kind:           i32,
    // latency
    pub lower_variance: Option<f32>,
    pub upper_variance: Option<f32>,
    pub duration:       Option<i32>,
    // throughput
    pub lower_events:   Option<f32>,
    pub upper_events:   Option<f32>,
    pub unit_time:      Option<i32>,
    // compute
    pub min_compute:    Option<f32>,
    pub max_compute:    Option<f32>,
    pub avg_compute:    Option<f32>,
    // memory
    pub min_memory:     Option<f32>,
    pub max_memory:     Option<f32>,
    pub avg_memory:     Option<f32>,
    // storage
    pub min_storage:    Option<f32>,
    pub max_storage:    Option<f32>,
    pub avg_storage:    Option<f32>,
}

// impl InsertPerf {
//     pub fn new(project_id: i32, name: String) -> Self {
//         Self {
//             uuid: Uuid::new_v4().to_string(),
//             project_id,
//             name,
//         }
//     }
// }
