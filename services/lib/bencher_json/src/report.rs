use std::collections::BTreeMap;

use chrono::{
    DateTime,
    Utc,
};
use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{
    Deserialize,
    Serialize,
};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewReport {
    pub branch:     Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash:       Option<String>,
    pub testbed:    Uuid,
    pub adapter:    JsonNewAdapter,
    pub start_time: DateTime<Utc>,
    pub end_time:   DateTime<Utc>,
    pub benchmarks: Vec<JsonNewBenchmarks>,
}

#[derive(Display, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum JsonNewAdapter {
    Json,
    #[display(fmt = "rust")]
    #[serde(rename = "rust")]
    RustCargoBench,
}

pub type JsonNewBenchmarks = BTreeMap<String, JsonNewPerf>;

#[derive(Debug, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewPerf {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency:    Option<JsonLatency>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub throughput: Option<JsonThroughput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compute:    Option<JsonMinMaxAvg>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory:     Option<JsonMinMaxAvg>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage:    Option<JsonMinMaxAvg>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonLatency {
    pub lower_variance: u128,
    pub upper_variance: u128,
    pub duration:       u128,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonThroughput {
    pub lower_events: f64,
    pub upper_events: f64,
    pub unit_time:    u128,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonMinMaxAvg {
    pub min: f64,
    pub max: f64,
    pub avg: f64,
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonReport {
    pub uuid:         Uuid,
    pub user_uuid:    Uuid,
    pub version_uuid: Uuid,
    pub testbed_uuid: Uuid,
    pub adapter_uuid: Uuid,
    pub start_time:   DateTime<Utc>,
    pub end_time:     DateTime<Utc>,
    pub benchmarks:   JsonBenchmarks,
}

pub type JsonBenchmarks = Vec<JsonBenchmarkPerf>;

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonBenchmarkPerf {
    pub benchmark_uuid: Uuid,
    pub perf_uuid:      Uuid,
}
