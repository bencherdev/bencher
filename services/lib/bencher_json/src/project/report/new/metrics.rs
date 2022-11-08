use std::collections::HashMap;

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{
    benchmarks::{CombinedKind, OrdKind},
    metric::JsonMetric,
};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonMetrics {
    #[serde(flatten)]
    pub inner: HashMap<MetricKind, JsonMetric>,
}

pub type MetricKind = String;

impl From<HashMap<MetricKind, JsonMetric>> for JsonMetrics {
    fn from(inner: HashMap<MetricKind, JsonMetric>) -> Self {
        Self { inner }
    }
}

impl JsonMetrics {
    pub(crate) fn combined(self, mut other: Self, kind: CombinedKind) -> Self {
        let mut metric_map = HashMap::new();
        for (metric_kind, metric) in self.inner.into_iter() {
            let other_metric = other.inner.remove(&metric_kind);
            let combined_metric = if let Some(other_metric) = other_metric {
                match kind {
                    CombinedKind::Ord(ord_kind) => match ord_kind {
                        OrdKind::Min => metric.min(other_metric),
                        OrdKind::Max => metric.max(other_metric),
                    },
                    CombinedKind::Add => metric + other_metric,
                }
            } else {
                metric
            };
            metric_map.insert(metric_kind, combined_metric);
        }
        for (metric_kind, other_metric) in other.inner.into_iter() {
            metric_map.insert(metric_kind, other_metric);
        }
        metric_map.into()
    }
}

impl std::ops::Div<usize> for JsonMetrics {
    type Output = Self;

    fn div(self, rhs: usize) -> Self::Output {
        let mut metric_map = HashMap::new();
        for (metric_kind, metric) in self.inner.into_iter() {
            metric_map.insert(metric_kind, metric / rhs);
        }
        metric_map.into()
    }
}
