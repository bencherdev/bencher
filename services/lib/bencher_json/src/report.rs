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
    #[serde(flatten)]
    pub benchmarks: JsonNewBenchmarks,
}

#[derive(Display, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum JsonNewAdapter {
    Json,
    #[display(fmt = "rust")]
    #[serde(rename = "rust")]
    RustCargoBench,
}

#[derive(Debug, Copy, Clone)]
pub enum OrdKind {
    Min,
    Max,
}

#[derive(Debug, Copy, Clone)]
pub enum AvgKind {
    Mean,
    Median,
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewBenchmarks {
    #[serde(rename = "benchmarks")]
    pub inner: Vec<JsonNewBenchmarksMap>,
}

impl From<Vec<JsonNewBenchmarksMap>> for JsonNewBenchmarks {
    fn from(benchmarks: Vec<JsonNewBenchmarksMap>) -> Self {
        Self { inner: benchmarks }
    }
}

impl JsonNewBenchmarks {
    pub fn ord(self, ord_kind: OrdKind) -> Self {
        let map = self.inner.into_iter().fold(
            BTreeMap::new().into(),
            |ord_map: JsonNewBenchmarksMap, next_map| ord_map.ord(next_map, ord_kind),
        );
        vec![map].into()
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewBenchmarksMap {
    #[serde(flatten)]
    pub inner: BTreeMap<String, JsonNewPerf>,
}

impl From<BTreeMap<String, JsonNewPerf>> for JsonNewBenchmarksMap {
    fn from(map: BTreeMap<String, JsonNewPerf>) -> Self {
        Self { inner: map }
    }
}

impl JsonNewBenchmarksMap {
    pub fn min(self, other: Self) -> Self {
        self.ord(other, OrdKind::Min)
    }

    pub fn max(self, other: Self) -> Self {
        self.ord(other, OrdKind::Max)
    }

    fn ord(self, mut other: Self, ord_kind: OrdKind) -> Self {
        let mut benchmarks_map = BTreeMap::new();
        for (benchmark_name, json_perf) in self.inner.into_iter() {
            let other_json_perf = other.inner.remove(&benchmark_name);
            let ord_json_perf = if let Some(other_json_perf) = other_json_perf {
                json_perf.ord(other_json_perf, ord_kind)
            } else {
                json_perf
            };
            benchmarks_map.insert(benchmark_name, ord_json_perf);
        }
        for (benchmark_name, other_json_perf) in other.inner.into_iter() {
            benchmarks_map.insert(benchmark_name, other_json_perf);
        }
        benchmarks_map.into()
    }
}

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
    fn ord(self, other: Self, ord_kind: OrdKind) -> Self {
        JsonNewPerf {
            latency:    ord_map(self.latency, other.latency, ord_kind),
            throughput: ord_map(self.throughput, other.throughput, ord_kind),
            compute:    ord_map(self.compute, other.compute, ord_kind),
            memory:     ord_map(self.memory, other.memory, ord_kind),
            storage:    ord_map(self.storage, other.storage, ord_kind),
        }
    }
}

fn ord_map<T>(self_perf: Option<T>, other_perf: Option<T>, ord_kind: OrdKind) -> Option<T>
where
    T: Ord,
{
    self_perf.map(|sp| {
        if let Some(op) = other_perf {
            match ord_kind {
                OrdKind::Min => sp.min(op),
                OrdKind::Max => sp.max(op),
            }
        } else {
            sp
        }
    })
}

#[derive(Debug, Default, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonLatency {
    pub lower_variance: u64,
    pub upper_variance: u64,
    pub duration:       u64,
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
    pub unit_time:      u64,
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
