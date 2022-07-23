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
pub struct NewReport {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project:    Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub testbed:    Option<Uuid>,
    pub adapter:    Adapter,
    pub start_time: DateTime<Utc>,
    pub end_time:   DateTime<Utc>,
    #[serde(flatten)]
    pub metrics:    Metrics,
}

impl NewReport {
    pub fn new(
        project: Option<String>,
        testbed: Option<Uuid>,
        adapter: Adapter,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        metrics: Metrics,
    ) -> Self {
        Self {
            project,
            testbed,
            adapter,
            start_time,
            end_time,
            metrics,
        }
    }
}

#[derive(Display, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum Adapter {
    Json,
    #[display(fmt = "rust")]
    #[serde(rename = "rust")]
    RustCargoBench,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct Metrics {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency:      Option<BTreeMap<Benchmark, Latency>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub throughput:   Option<BTreeMap<Benchmark, ()>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_cpu:    Option<BTreeMap<Benchmark, ()>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub self_cpu:     Option<BTreeMap<Benchmark, ()>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_memory: Option<BTreeMap<Benchmark, ()>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub self_memory:  Option<BTreeMap<Benchmark, ()>>,
}

pub type Benchmark = String;

#[derive(Debug, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct Latency {
    pub duration:       Duration,
    pub upper_variance: Duration,
    pub lower_variance: Duration,
}
