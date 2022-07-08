use std::collections::BTreeMap;
use std::time::Duration;

use chrono::{DateTime, Utc};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct Report {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub testbed: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: DateTime<Utc>,
    #[serde(flatten)]
    pub metrics: Metrics,
}

impl Report {
    pub fn new(
        project: Option<String>,
        testbed: Option<String>,
        start_time: Option<DateTime<Utc>>,
        end_time: DateTime<Utc>,
        metrics: Metrics,
    ) -> Self {
        Self {
            project,
            testbed,
            start_time,
            end_time,
            metrics,
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct Metrics {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency: Option<BTreeMap<Benchmark, Latency>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub throughput: Option<BTreeMap<Benchmark, ()>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_cpu: Option<BTreeMap<Benchmark, ()>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub self_cpu: Option<BTreeMap<Benchmark, ()>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_memory: Option<BTreeMap<Benchmark, ()>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub self_memory: Option<BTreeMap<Benchmark, ()>>,
}

pub type Benchmark = String;

#[derive(Debug, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct Latency {
    pub duration: Duration,
    pub upper_variance: Duration,
    pub lower_variance: Duration,
}
