use std::collections::{BTreeMap, HashMap};

use super::{
    benchmarks::{JsonBenchmarks, JsonBenchmarksMap},
    median::Median,
    metric::JsonMetric,
    metrics::JsonMetrics,
};

#[derive(Debug, Clone, Default)]
pub struct JsonMetricsMap {
    pub inner: HashMap<String, JsonMetricsList>,
}

impl From<JsonBenchmarks> for JsonMetricsMap {
    fn from(benchmarks: JsonBenchmarks) -> Self {
        let mut perf_list_map = Self::default();
        for benchmarks_map in benchmarks.inner.into_iter() {
            perf_list_map.append(benchmarks_map);
        }
        perf_list_map
    }
}

impl JsonMetricsMap {
    fn append(&mut self, benchmarks_map: JsonBenchmarksMap) {
        for (benchmark_name, metrics) in benchmarks_map.inner.into_iter() {
            if let Some(metrics_list) = self.inner.get_mut(&benchmark_name) {
                for (metric_kind, metric) in metrics.inner {
                    if let Some(list) = metrics_list.inner.get_mut(&metric_kind) {
                        list.push(metric);
                    } else {
                        metrics_list.inner.insert(metric_kind, vec![metric]);
                    }
                }
            } else {
                let mut metrics_list = HashMap::new();
                for (metric_kind, metric) in metrics.inner {
                    metrics_list.insert(metric_kind, vec![metric]);
                }
                self.inner.insert(
                    benchmark_name,
                    JsonMetricsList {
                        inner: metrics_list,
                    },
                );
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct JsonMetricsList {
    pub inner: HashMap<String, Vec<JsonMetric>>,
}

impl JsonMetricsList {
    pub(crate) fn median(self) -> JsonMetrics {
        let mut metric_map = BTreeMap::new();
        for (metric_kind, metric) in self.inner.into_iter() {
            if let Some(median) = JsonMetric::median(metric) {
                metric_map.insert(metric_kind, median);
            }
        }
        metric_map.into()
    }
}
