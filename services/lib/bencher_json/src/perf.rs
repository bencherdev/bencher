use chrono::NaiveDateTime;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{
    Deserialize,
    Serialize,
};
use uuid::Uuid;

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
pub struct JsonPerf<T>(Vec<JsonPerfData<T>>);


#[derive(Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPerfData<T> {
    pub branch_uuid: Uuid,
    pub testbed_uuid: Uuid,
    pub benchmark_uuid: Uuid,
    pub kind:       JsonPerfKind,
    pub start_time: Option<NaiveDateTime>,
    pub end_time:   Option<NaiveDateTime>,
    pub data: Vec<JsonPerfDatum<T>>,
}

#[derive(Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPerfDatum<T> {
    pub perf_uuid: Uuid,
    pub start_time: NaiveDateTime,
    pub end_time: NaiveDateTime,
    pub version_number: u32,
    pub version_hash: String,
    pub perf: T
}