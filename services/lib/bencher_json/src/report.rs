use std::{
    cmp::Ordering,
    collections::BTreeMap,
};

use chrono::{
    DateTime,
    Utc,
};
use derive_more::Display;
use ordered_float::OrderedFloat;
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

impl JsonNewPerf {
    pub fn min(self, other: Self) -> Self {
        JsonNewPerf {
            latency:    min_map(self.latency, other.latency),
            throughput: min_map(self.throughput, other.throughput),
            compute:    min_map(self.compute, other.compute),
            memory:     min_map(self.memory, other.memory),
            storage:    min_map(self.storage, other.storage),
        }
    }

    pub fn max(self, other: Self) -> Self {
        JsonNewPerf {
            latency:    max_map(self.latency, other.latency),
            throughput: max_map(self.throughput, other.throughput),
            compute:    max_map(self.compute, other.compute),
            memory:     max_map(self.memory, other.memory),
            storage:    max_map(self.storage, other.storage),
        }
    }
}

fn min_map<T>(self_perf: Option<T>, other_perf: Option<T>) -> Option<T>
where
    T: Ord,
{
    self_perf.map(|sp| {
        if let Some(op) = other_perf {
            sp.min(op)
        } else {
            sp
        }
    })
}

fn max_map<T>(self_perf: Option<T>, other_perf: Option<T>) -> Option<T>
where
    T: Ord,
{
    self_perf.map(|sp| {
        if let Some(op) = other_perf {
            sp.max(op)
        } else {
            sp
        }
    })
}

#[derive(Debug, Default, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonLatency {
    pub lower_variance: u128,
    pub upper_variance: u128,
    pub duration:       u128,
}

impl PartialEq for JsonLatency {
    fn eq(&self, other: &Self) -> bool {
        self.lower_variance == other.lower_variance
            && self.upper_variance == other.upper_variance
            && self.duration == other.duration
    }
}

impl PartialOrd for JsonLatency {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for JsonLatency {
    fn cmp(&self, other: &Self) -> Ordering {
        let duration_order = self.duration.cmp(&other.duration);
        if Ordering::Equal == duration_order {
            let upper_order = self.upper_variance.cmp(&other.upper_variance);
            if Ordering::Equal == upper_order {
                self.lower_variance.cmp(&other.lower_variance)
            } else {
                upper_order
            }
        } else {
            duration_order
        }
    }
}

#[derive(Debug, Default, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonThroughput {
    pub lower_variance: OrderedFloat<f64>,
    pub upper_variance: OrderedFloat<f64>,
    pub events:         OrderedFloat<f64>,
    pub unit_time:      u128,
}

impl PartialEq for JsonThroughput {
    fn eq(&self, other: &Self) -> bool {
        self.lower_variance == other.lower_variance
            && self.upper_variance == other.upper_variance
            && self.events == other.events
            && self.unit_time == other.unit_time
    }
}

impl PartialOrd for JsonThroughput {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for JsonThroughput {
    fn cmp(&self, other: &Self) -> Ordering {
        let events_order = self
            .per_unit_time(&self.events)
            .cmp(&other.per_unit_time(&other.events));
        if Ordering::Equal == events_order {
            let upper_order = self
                .per_unit_time(&self.upper_variance)
                .cmp(&other.per_unit_time(&other.upper_variance));
            if Ordering::Equal == upper_order {
                self.per_unit_time(&self.lower_variance)
                    .cmp(&other.per_unit_time(&other.lower_variance))
            } else {
                upper_order
            }
        } else {
            events_order
        }
    }
}

impl JsonThroughput {
    fn per_unit_time(&self, events: &OrderedFloat<f64>) -> OrderedFloat<f64> {
        OrderedFloat(events.into_inner() / self.unit_time as f64)
    }
}

#[derive(Debug, Default, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonMinMaxAvg {
    pub min: OrderedFloat<f64>,
    pub max: OrderedFloat<f64>,
    pub avg: OrderedFloat<f64>,
}

impl PartialEq for JsonMinMaxAvg {
    fn eq(&self, other: &Self) -> bool {
        self.min == other.min && self.max == other.max && self.avg == other.avg
    }
}

impl PartialOrd for JsonMinMaxAvg {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for JsonMinMaxAvg {
    fn cmp(&self, other: &Self) -> Ordering {
        let avg_order = self.avg.cmp(&other.avg);
        if Ordering::Equal == avg_order {
            let max_order = self.max.cmp(&other.max);
            if Ordering::Equal == max_order {
                self.min.cmp(&other.min)
            } else {
                max_order
            }
        } else {
            avg_order
        }
    }
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
