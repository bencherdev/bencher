#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{
    benchmarks::OrdKind, latency::JsonLatency, resource::JsonResource, throughput::JsonThroughput,
};

#[derive(Debug, Copy, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonMetrics {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency: Option<JsonLatency>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub throughput: Option<JsonThroughput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compute: Option<JsonResource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<JsonResource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage: Option<JsonResource>,
}

impl JsonMetrics {
    pub(crate) fn ord(self, other: Self, ord_kind: OrdKind) -> Self {
        JsonMetrics {
            latency: ord_map(self.latency, other.latency, ord_kind),
            throughput: ord_map(self.throughput, other.throughput, ord_kind),
            compute: ord_map(self.compute, other.compute, ord_kind),
            memory: ord_map(self.memory, other.memory, ord_kind),
            storage: ord_map(self.storage, other.storage, ord_kind),
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

impl std::ops::Add for JsonMetrics {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            latency: add_map(self.latency, other.latency),
            throughput: add_map(self.throughput, other.throughput),
            compute: add_map(self.compute, other.compute),
            memory: add_map(self.memory, other.memory),
            storage: add_map(self.storage, other.storage),
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

impl std::ops::Div<usize> for JsonMetrics {
    type Output = Self;

    fn div(self, rhs: usize) -> Self::Output {
        Self {
            latency: div_map(self.latency, rhs),
            throughput: div_map(self.throughput, rhs),
            compute: div_map(self.compute, rhs),
            memory: div_map(self.memory, rhs),
            storage: div_map(self.storage, rhs),
        }
    }
}

fn div_map<T>(self_perf: Option<T>, rhs: usize) -> Option<T>
where
    T: std::ops::Div<usize, Output = T>,
{
    self_perf.map(|sp| sp / rhs)
}
