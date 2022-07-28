use std::{
    collections::BTreeMap,
    time::Duration,
};

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
pub struct JsonReport {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project:    Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub testbed:    Option<Uuid>,
    pub adapter:    JsonAdapter,
    pub start_time: DateTime<Utc>,
    pub end_time:   DateTime<Utc>,
    pub benchmarks: JsonBenchmarks,
    // TODO add a tags section, for noting things like code version etc
    // the CLI could have `--tag-version` flag that would automatically look for the current git
    // hash and add that under the `version` tag.
}

#[derive(Display, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum JsonAdapter {
    Json,
    #[display(fmt = "rust")]
    #[serde(rename = "rust")]
    RustCargoBench,
}

pub type JsonBenchmarks = BTreeMap<String, JsonBenchmark>;

#[derive(Debug, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonBenchmark {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency:    Option<JsonLatency>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub throughput: Option<JsonThroughput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu:        Option<JsonCpu>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory:     Option<JsonMemory>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonLatency {
    pub duration:       Duration,
    pub lower_variance: Duration,
    pub upper_variance: Duration,
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
pub struct JsonCpu {
    pub min: f64,
    pub max: f64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonMemory {
    pub min: f64,
    pub max: f64,
}
