use std::{
    cmp::Ordering,
    collections::BTreeMap,
};

use chrono::{
    DateTime,
    Utc,
};
use derive_more::{
    Add,
    Display,
    Sum,
};
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
enum OrdKind {
    Min,
    Max,
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

type JsonNewPerfListMap = BTreeMap<String, JsonNewPerfList>;

impl JsonNewBenchmarks {
    pub fn min(self) -> Self {
        self.ord(OrdKind::Min)
    }

    pub fn max(self) -> Self {
        self.ord(OrdKind::Max)
    }

    fn ord(self, ord_kind: OrdKind) -> Self {
        let map = self.inner.into_iter().fold(
            BTreeMap::new().into(),
            |ord_map: JsonNewBenchmarksMap, next_map| {
                ord_map.combined(next_map, CombinedKind::Ord(ord_kind))
            },
        );
        vec![map].into()
    }

    pub fn mean(self) -> Self {
        let length = self.inner.len();
        let map: JsonNewBenchmarksMap = self.inner.into_iter().sum();
        vec![map / length].into()
    }

    pub fn median(self) -> Self {
        let mut benchmarks_list_map: JsonNewPerfListMap = BTreeMap::new();
        for benchmarks_map in self.inner.into_iter() {
            benchmarks_map.append_to(&mut benchmarks_list_map);
        }
        vec![benchmarks_list_map
            .into_iter()
            .map(|(benchmark_name, json_perf_list)| (benchmark_name, json_perf_list.into()))
            .collect::<BTreeMap<String, JsonNewPerf>>()
            .into()]
        .into()
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

enum CombinedKind {
    Ord(OrdKind),
    Add,
}

impl JsonNewBenchmarksMap {
    fn combined(self, mut other: Self, kind: CombinedKind) -> Self {
        let mut benchmarks_map = BTreeMap::new();
        for (benchmark_name, json_perf) in self.inner.into_iter() {
            let other_json_perf = other.inner.remove(&benchmark_name);
            let ord_json_perf = if let Some(other_json_perf) = other_json_perf {
                match kind {
                    CombinedKind::Ord(ord_kind) => json_perf.ord(other_json_perf, ord_kind),
                    CombinedKind::Add => json_perf + other_json_perf,
                }
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

    fn append_to(self, benchmarks_list_map: &mut JsonNewPerfListMap) {
        for (benchmark_name, json_perf) in self.inner.into_iter() {
            let JsonNewPerf {
                latency,
                throughput,
                compute,
                memory,
                storage,
            } = json_perf;
            if let Some(json_perfs) = benchmarks_list_map.get_mut(&benchmark_name) {
                json_perfs.latency.push(latency);
                json_perfs.throughput.push(throughput);
                json_perfs.compute.push(compute);
                json_perfs.memory.push(memory);
                json_perfs.storage.push(storage);
            } else {
                benchmarks_list_map.insert(benchmark_name, JsonNewPerfList {
                    latency:    vec![latency],
                    throughput: vec![throughput],
                    compute:    vec![compute],
                    memory:     vec![memory],
                    storage:    vec![storage],
                });
            }
        }
    }
}

impl std::ops::Add for JsonNewBenchmarksMap {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        self.combined(other, CombinedKind::Add)
    }
}

impl std::iter::Sum for JsonNewBenchmarksMap {
    fn sum<I>(iter: I) -> Self
    where
        I: Iterator<Item = Self>,
    {
        iter.into_iter().fold(
            BTreeMap::new().into(),
            |acc_map: JsonNewBenchmarksMap, next_map| acc_map + next_map,
        )
    }
}

impl std::ops::Div<usize> for JsonNewBenchmarksMap {
    type Output = Self;

    fn div(mut self, rhs: usize) -> Self::Output {
        for (_, json_perf) in self.inner.iter_mut() {
            *json_perf = *json_perf / rhs;
        }
        self
    }
}

#[derive(Debug, Copy, Clone, Default, Serialize, Deserialize)]
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

impl std::ops::Add for JsonNewPerf {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            latency:    add_map(self.latency, other.latency),
            throughput: add_map(self.throughput, other.throughput),
            compute:    add_map(self.compute, other.compute),
            memory:     add_map(self.memory, other.memory),
            storage:    add_map(self.storage, other.storage),
        }
    }
}

fn add_map<T>(self_perf: Option<T>, other_perf: Option<T>) -> Option<T>
where
    T: std::ops::Add<Output = T>,
{
    self_perf.map(|sp| {
        if let Some(op) = other_perf {
            sp + op
        } else {
            sp
        }
    })
}

impl std::ops::Div<usize> for JsonNewPerf {
    type Output = Self;

    fn div(self, rhs: usize) -> Self::Output {
        Self {
            latency:    div_map(self.latency, rhs),
            throughput: div_map(self.throughput, rhs),
            compute:    div_map(self.compute, rhs),
            memory:     div_map(self.memory, rhs),
            storage:    div_map(self.storage, rhs),
        }
    }
}

fn div_map<T>(self_perf: Option<T>, rhs: usize) -> Option<T>
where
    T: std::ops::Div<usize, Output = T>,
{
    self_perf.map(|sp| sp / rhs)
}

struct JsonNewPerfList {
    pub latency:    Vec<Option<JsonLatency>>,
    pub throughput: Vec<Option<JsonThroughput>>,
    pub compute:    Vec<Option<JsonMinMaxAvg>>,
    pub memory:     Vec<Option<JsonMinMaxAvg>>,
    pub storage:    Vec<Option<JsonMinMaxAvg>>,
}

impl From<JsonNewPerfList> for JsonNewPerf {
    fn from(json_perf_list: JsonNewPerfList) -> Self {
        let JsonNewPerfList {
            latency,
            throughput,
            compute,
            memory,
            storage,
        } = json_perf_list;
        Self {
            latency:    JsonLatency::median(latency),
            throughput: JsonThroughput::median(throughput),
            compute:    JsonMinMaxAvg::median(compute),
            memory:     JsonMinMaxAvg::median(memory),
            storage:    JsonMinMaxAvg::median(storage),
        }
    }
}

trait Median {
    fn median(mut array: Vec<Option<Self>>) -> Option<Self>
    where
        Self: Copy
            + Clone
            + Ord
            + std::ops::Add<Output = Self>
            + std::ops::Div<usize, Output = Self>
            + std::default::Default,
    {
        array.sort_unstable();

        let size = array.len();
        if (size % 2) == 0 {
            let left = size / 2 - 1;
            let right = size / 2;
            Some((array[left]? + array[right]?) / 2)
        } else {
            array[(size / 2)]
        }
    }
}

#[derive(Debug, Copy, Clone, Default, Eq, Add, Sum, Serialize, Deserialize)]
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

impl std::ops::Div<usize> for JsonLatency {
    type Output = Self;

    fn div(self, rhs: usize) -> Self::Output {
        Self {
            lower_variance: self.lower_variance / rhs as u64,
            upper_variance: self.upper_variance / rhs as u64,
            duration:       self.duration / rhs as u64,
        }
    }
}

impl Median for JsonLatency {}

// impl Median for JsonLatency {
//     fn median(mut array: Vec<Option<Self>>) -> Self {
//         array.sort_unstable();

//         if (array.len() % 2) == 0 {
//             let ind_left = array.len() / 2 - 1;
//             let ind_right = array.len() / 2;
//             (array[ind_left].unwrap_or_default() +
// array[ind_right].unwrap_or_default()) / 2         } else {
//             array[(array.len() / 2)].unwrap_or_default()
//         }
//     }
// }

#[derive(Debug, Copy, Clone, Default, Eq, Add, Sum, Serialize, Deserialize)]
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

impl std::ops::Div<usize> for JsonThroughput {
    type Output = Self;

    fn div(self, rhs: usize) -> Self::Output {
        Self {
            lower_variance: self.lower_variance / rhs as f64,
            upper_variance: self.upper_variance / rhs as f64,
            events:         self.events / rhs as f64,
            unit_time:      self.unit_time / rhs as u64,
        }
    }
}

impl Median for JsonThroughput {}

#[derive(Debug, Copy, Clone, Default, Eq, Add, Sum, Serialize, Deserialize)]
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

impl std::ops::Div<usize> for JsonMinMaxAvg {
    type Output = Self;

    fn div(self, rhs: usize) -> Self::Output {
        Self {
            min: self.min / rhs as f64,
            max: self.max / rhs as f64,
            avg: self.avg / rhs as f64,
        }
    }
}

impl Median for JsonMinMaxAvg {}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonReport {
    pub uuid:       Uuid,
    pub user:       Uuid,
    pub version:    Uuid,
    pub testbed:    Uuid,
    pub adapter:    Uuid,
    pub start_time: DateTime<Utc>,
    pub end_time:   DateTime<Utc>,
    pub benchmarks: JsonBenchmarks,
    pub alerts:     JsonAlerts,
}

pub type JsonBenchmarks = Vec<JsonBenchmarkPerf>;

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonBenchmarkPerf {
    pub benchmark: Uuid,
    pub perf:      Uuid,
}

pub type JsonAlerts = Vec<JsonPerfAlert>;

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPerfAlert {
    pub perf:  Uuid,
    pub alert: Uuid,
}
