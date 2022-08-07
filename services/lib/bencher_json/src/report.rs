use std::{
    collections::HashMap,
    time::Duration,
};

use chrono::{
    DateTime,
    NaiveDateTime,
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
    pub adapter:    JsonAdapter,
    pub start_time: DateTime<Utc>,
    pub end_time:   DateTime<Utc>,
    pub benchmarks: JsonBenchmarks,
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonReport {
    pub uuid:         Uuid,
    pub user_uuid:    Uuid,
    pub version_uuid: Uuid,
    pub testbed_uuid: Uuid,
    pub adapter_uuid: Uuid,
    pub start_time:   NaiveDateTime,
    pub end_time:     NaiveDateTime,
    pub benchmarks:   JsonBenchmarks,
}

#[derive(Display, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum JsonAdapter {
    Json,
    #[display(fmt = "rust")]
    #[serde(rename = "rust")]
    RustCargoBench,
}

pub type JsonBenchmarks = HashMap<String, JsonPerf>;

#[derive(Debug, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPerf {
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
    pub lower_variance: Duration,
    pub upper_variance: Duration,
    pub duration:       Duration,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonThroughput {
    pub lower_events: f64,
    pub upper_events: f64,
    pub unit_time:    Duration,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonMinMaxAvg {
    pub min: f64,
    pub max: f64,
    pub avg: f64,
}
