use std::collections::HashMap;

use bencher_json::JsonMetric;
use serde::{Deserialize, Serialize};

use super::{CombinedKind, MetricKind, OrdKind};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterMetrics {
    #[serde(flatten)]
    pub inner: MetricsMap,
}

pub type MetricsMap = HashMap<MetricKind, JsonMetric>;

impl From<MetricsMap> for AdapterMetrics {
    fn from(inner: MetricsMap) -> Self {
        Self { inner }
    }
}

impl AdapterMetrics {
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
        metric_map.extend(other.inner.into_iter());
        metric_map.into()
    }
}

impl std::ops::Div<usize> for AdapterMetrics {
    type Output = Self;

    fn div(self, rhs: usize) -> Self::Output {
        let mut metric_map = HashMap::new();
        for (metric_kind, metric) in self.inner.into_iter() {
            metric_map.insert(metric_kind, metric / rhs);
        }
        metric_map.into()
    }
}
