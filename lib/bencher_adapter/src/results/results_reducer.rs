use std::collections::HashMap;

use bencher_json::{project::metric::Median, BenchmarkName, JsonMetric};

use super::{
    adapter_metrics::AdapterMetrics, adapter_results::AdapterResults, AdapterResultsArray, Measure,
};

#[derive(Debug, Clone, Default)]
pub struct ResultsReducer {
    pub inner: HashMap<BenchmarkName, MeasuresMap>,
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
            if let Some(measures_map) = self.inner.get_mut(&benchmark_name) {
                for (measure, metric) in metrics.inner {
                    if let Some(list) = measures_map.inner.get_mut(&measure) {
                        list.push(metric);
                    } else {
                        measures_map.inner.insert(measure, vec![metric]);
                    }
                }
            } else {
                let mut measures_map = HashMap::new();
                for (measure, metric) in metrics.inner {
                    measures_map.insert(measure, vec![metric]);
                }
                self.inner.insert(
                    benchmark_name,
                    MeasuresMap {
                        inner: measures_map,
                    },
                );
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct MeasuresMap {
    pub inner: HashMap<Measure, Vec<JsonMetric>>,
}

impl MeasuresMap {
    pub(crate) fn median(self) -> AdapterMetrics {
        let mut metric_map = HashMap::new();
        for (measure, metric) in self.inner {
            if let Some(median) = JsonMetric::median(metric) {
                metric_map.insert(measure, median);
            }
        }
        metric_map.into()
    }
}
