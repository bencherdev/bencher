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
        let events_per_unit_time = OrderedFloat(self.events.into_inner() / self.unit_time as f64);
        let other_events_per_unit_time =
            OrderedFloat(other.events.into_inner() / other.unit_time as f64);

        let events_order = events_per_unit_time.cmp(&other_events_per_unit_time);
        if Ordering::Equal == events_order {
            let upper_order = self.upper_variance.cmp(&other.upper_variance);
            if Ordering::Equal == upper_order {
                self.lower_variance.cmp(&other.lower_variance)
            } else {
                upper_order
            }
        } else {
            events_order
        }
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
