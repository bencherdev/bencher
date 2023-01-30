use std::collections::HashMap;

use bencher_json::{project::metric::Median, BenchmarkName, JsonMetric};

use super::{
    adapter_metrics::AdapterMetrics, adapter_results::AdapterResults, AdapterResultsArray,
    MetricKind,
};

#[derive(Debug, Clone, Default)]
pub struct ResultsReducer {
    pub inner: HashMap<BenchmarkName, MetricKindMap>,
}

impl From<AdapterResultsArray> for ResultsReducer {
    fn from(results_array: AdapterResultsArray) -> Self {
        let mut results_reducer = Self::default();
        for results in results_array.inner {
            results_reducer.reduce(results);
        }
        results_reducer
    }
}

impl ResultsReducer {
    fn reduce(&mut self, results: AdapterResults) {
        for (benchmark_name, metrics) in results.inner {
            if let Some(metric_kind_map) = self.inner.get_mut(&benchmark_name) {
                for (metric_kind, metric) in metrics.inner {
                    if let Some(list) = metric_kind_map.inner.get_mut(&metric_kind) {
                        list.push(metric);
                    } else {
                        metric_kind_map.inner.insert(metric_kind, vec![metric]);
                    }
                }
            } else {
                let mut metric_kind_map = HashMap::new();
                for (metric_kind, metric) in metrics.inner {
                    metric_kind_map.insert(metric_kind, vec![metric]);
                }
                self.inner.insert(
                    benchmark_name,
                    MetricKindMap {
                        inner: metric_kind_map,
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
        for (metric_kind, metric) in self.inner {
            if let Some(median) = JsonMetric::median(metric) {
                metric_map.insert(metric_kind, median);
            }
        }
        metric_map.into()
    }
}
