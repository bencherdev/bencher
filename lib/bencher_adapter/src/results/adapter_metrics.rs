use std::{collections::HashMap, str::FromStr};

use bencher_json::{JsonMetric, MeasureNameId};
use serde::{Deserialize, Serialize};

use super::{CombinedKind, OrdKind};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterMetrics {
    #[serde(flatten)]
    pub inner: MetricsMap,
}

pub type MetricsMap = HashMap<MeasureNameId, JsonMetric>;

impl From<MetricsMap> for AdapterMetrics {
    fn from(inner: MetricsMap) -> Self {
        Self { inner }
    }
}

impl AdapterMetrics {
    pub(crate) fn combined(self, mut other: Self, kind: CombinedKind) -> Self {
        let mut metric_map = HashMap::new();
        for (measure, metric) in self.inner {
            let other_metric = other.inner.remove(&measure);
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
            metric_map.insert(measure, combined_metric);
        }
        metric_map.extend(other.inner);
        metric_map.into()
    }

    pub fn get(&self, key: &str) -> Option<&JsonMetric> {
        self.inner.get(&MeasureNameId::from_str(key).ok()?)
    }
}

impl std::ops::Div<usize> for AdapterMetrics {
    type Output = Self;

    fn div(self, rhs: usize) -> Self::Output {
        let mut metric_map = HashMap::new();
        for (measure, metric) in self.inner {
            metric_map.insert(measure, metric / rhs);
        }
        metric_map.into()
    }
}
