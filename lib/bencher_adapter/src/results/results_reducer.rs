use std::collections::HashMap;

use bencher_json::{BenchmarkNameId, JsonNewMetric, MeasureNameId, project::metric::Median as _};

use super::foldable::{FoldableMetrics, FoldableResults, FoldableResultsArray};

#[derive(Debug, Clone, Default)]
pub struct ResultsReducer {
    pub inner: HashMap<BenchmarkNameId, MeasuresMap>,
}

impl From<FoldableResultsArray> for ResultsReducer {
    fn from(results_array: FoldableResultsArray) -> Self {
        let mut results_reducer = Self::default();
        for results in results_array.inner {
            results_reducer.reduce(results);
        }
        results_reducer
    }
}

impl ResultsReducer {
    fn reduce(&mut self, results: FoldableResults) {
        for (benchmark, metrics) in results.inner {
            if let Some(measures_map) = self.inner.get_mut(&benchmark) {
                for (measure, metric) in metrics {
                    if let Some(list) = measures_map.inner.get_mut(&measure) {
                        list.push(metric);
                    } else {
                        measures_map.inner.insert(measure, vec![metric]);
                    }
                }
            } else {
                let mut measures_map = HashMap::new();
                for (measure, metric) in metrics {
                    measures_map.insert(measure, vec![metric]);
                }
                self.inner.insert(
                    benchmark,
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
    pub inner: HashMap<MeasureNameId, Vec<JsonNewMetric>>,
}

impl MeasuresMap {
    pub(crate) fn median(self) -> FoldableMetrics {
        let mut metric_map = FoldableMetrics::new();
        for (measure, metric) in self.inner {
            if let Some(median) = JsonNewMetric::median(metric) {
                metric_map.insert(measure, median);
            }
        }
        metric_map
    }
}
