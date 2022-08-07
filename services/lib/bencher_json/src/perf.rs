use chrono::NaiveDateTime;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{
    Deserialize,
    Serialize,
};
use uuid::Uuid;

use crate::report::{
    JsonLatency,
    JsonMinMaxAvg,
    JsonThroughput,
};

#[derive(Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPerfQuery {
    pub branches:   Vec<Uuid>,
    pub testbeds:   Vec<Uuid>,
    pub benchmarks: Vec<Uuid>,
    pub kind:       JsonPerfKind,
    pub start_time: Option<NaiveDateTime>,
    pub end_time:   Option<NaiveDateTime>,
}

#[derive(Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum JsonPerfKind {
    Latency,
    Throughput,
    Compute,
    Memory,
    Storage,
}

#[derive(Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPerf {
    pub kind:       JsonPerfKind,
    pub start_time: Option<NaiveDateTime>,
    pub end_time:   Option<NaiveDateTime>,
    pub data:       Vec<JsonPerfData>,
}

#[derive(Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPerfData {
    pub branch_uuid:    Uuid,
    pub testbed_uuid:   Uuid,
    pub benchmark_uuid: Uuid,
    pub data:           Vec<JsonPerfDatum>,
}

#[derive(Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPerfDatum {
    pub perf_uuid:      Uuid,
    pub start_time:     NaiveDateTime,
    pub end_time:       NaiveDateTime,
    pub version_number: u32,
    pub version_hash:   Option<String>,
    pub datum:          JsonPerfDatumKind,
}

#[derive(Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum JsonPerfDatumKind {
    Latency(JsonLatency),
    Throughput(JsonThroughput),
    Compute(JsonMinMaxAvg),
    Memory(JsonMinMaxAvg),
    Storage(JsonMinMaxAvg),
}
