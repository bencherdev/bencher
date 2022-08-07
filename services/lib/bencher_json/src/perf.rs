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
pub struct JsonPerf {
    pub data: Vec<()>,
}
