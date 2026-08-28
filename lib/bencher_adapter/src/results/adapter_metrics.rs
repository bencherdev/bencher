use std::{
    collections::{BTreeMap, HashMap},
    str::FromStr as _,
};

use bencher_json::{JsonNewMetric, MeasureNameId, MetricName};
use ordered_float::OrderedFloat;

/// Every measure a benchmark reported at one variant.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AdapterMetrics {
    pub inner: MetricsMap,
}

pub type MetricsMap = HashMap<MeasureNameId, AdapterMetric>;

impl From<MetricsMap> for AdapterMetrics {
    fn from(inner: MetricsMap) -> Self {
        Self { inner }
    }
}

impl AdapterMetrics {
    /// The metric triple a measure's conventional names spell out,
    /// or `None` if the measure never named a point estimate.
    pub fn get(&self, key: &str) -> Option<JsonNewMetric> {
        self.inner
            .get(&MeasureNameId::from_str(key).ok()?)?
            .triple()
    }
}

/// One measure's named scalars.
///
/// Every name is equal here. `value`, `lower_value`, and `upper_value` are
/// conventional, not privileged: they are the names a BMF v0 metric triple maps
/// onto, and the names the console knows well enough to draw a band. A BMF v1
/// measure may carry only `p99` and never mention `value` at all.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AdapterMetric {
    pub inner: NamedMap,
}

/// Named scalars in lexicographic order, which is the order the cap keeps.
pub type NamedMap = BTreeMap<MetricName, OrderedFloat<f64>>;

/// The most named values one measure may carry, anchored to the number of
/// statistics k6's `summary_trend_stats` reports by default.
///
/// Deliberately low: raising the cap is a release note and lowering it is a
/// breaking change, so the asymmetry runs one way.
pub const MAX_METRIC_NAMES: usize = 8;

impl From<NamedMap> for AdapterMetric {
    fn from(inner: NamedMap) -> Self {
        Self { inner }
    }
}

/// A metric triple becomes exactly the three conventional names,
/// and a metric without bounds becomes exactly one.
impl From<JsonNewMetric> for AdapterMetric {
    fn from(metric: JsonNewMetric) -> Self {
        let JsonNewMetric {
            value,
            lower_value,
            upper_value,
        } = metric;
        let mut inner = NamedMap::new();
        inner.insert(MetricName::value(), value);
        if let Some(lower_value) = lower_value {
            inner.insert(MetricName::lower_value(), lower_value);
        }
        if let Some(upper_value) = upper_value {
            inner.insert(MetricName::upper_value(), upper_value);
        }
        Self { inner }
    }
}

impl AdapterMetric {
    /// The metric triple these names spell out,
    /// or `None` if there is no `value` name to be the point estimate.
    pub fn triple(&self) -> Option<JsonNewMetric> {
        Some(JsonNewMetric {
            value: *self.inner.get(&MetricName::value())?,
            lower_value: self.inner.get(&MetricName::lower_value()).copied(),
            upper_value: self.inner.get(&MetricName::upper_value()).copied(),
        })
    }

    /// Drop named values beyond [`MAX_METRIC_NAMES`], returning how many were dropped.
    ///
    /// Survival is deterministic because hash map iteration order is not an
    /// acceptable tiebreak: the three conventional names are never dropped, and
    /// the remainder is kept in lexicographic order up to the cap.
    pub(crate) fn truncate(&mut self) -> usize {
        if self.inner.len() <= MAX_METRIC_NAMES {
            return 0;
        }

        let conventional = [
            MetricName::value(),
            MetricName::lower_value(),
            MetricName::upper_value(),
        ];
        let mut budget = MAX_METRIC_NAMES.saturating_sub(
            conventional
                .iter()
                .filter(|name| self.inner.contains_key(name))
                .count(),
        );

        let mut dropped = 0;
        self.inner.retain(|name, _| {
            if conventional.contains(name) {
                true
            } else if budget > 0 {
                budget -= 1;
                true
            } else {
                dropped += 1;
                false
            }
        });
        dropped
    }
}
