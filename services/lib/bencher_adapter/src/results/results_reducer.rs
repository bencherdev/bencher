use std::collections::HashMap;

use bencher_json::{project::metric::Median, JsonMetric};

use super::{
    adapter_metrics::AdapterMetrics, adapter_results::AdapterResults, AdapterResultsArray,
    BenchmarkName, MetricKind,
};

#[derive(Debug, Clone, Default)]
pub struct ResultsReducer {
    pub inner: HashMap<BenchmarkName, MetricKindMap>,
}

impl From<AdapterResultsArray> for ResultsReducer {
    fn from(benchmarks: AdapterResultsArray) -> Self {
        let mut perf_list_map = Self::default();
        for benchmarks_map in benchmarks.inner.into_iter() {
            perf_list_map.reduce(benchmarks_map);
        }
        perf_list_map
    }
}

impl ResultsReducer {
    fn reduce(&mut self, benchmarks_map: AdapterResults) {
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
                    MetricKindMap {
                        inner: metrics_list,
                    },
                );
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct MetricKindMap {
    pub inner: HashMap<MetricKind, Vec<JsonMetric>>,
}

impl MetricKindMap {
    pub(crate) fn median(self) -> AdapterMetrics {
        let mut metric_map = HashMap::new();
        for (metric_kind, metric) in self.inner.into_iter() {
            if let Some(median) = JsonMetric::median(metric) {
                metric_map.insert(metric_kind, median);
            }
        }
        metric_map.into()
    }
}
