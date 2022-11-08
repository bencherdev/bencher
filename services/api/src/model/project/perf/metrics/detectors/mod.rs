use std::collections::HashMap;

use bencher_json::project::report::{new::JsonBenchmarks, JsonMetric, JsonMetricsMap};
use diesel::SqliteConnection;
use dropshot::HttpError;

use self::detector::Detector;
pub use self::threshold::Threshold;
use crate::{
    model::project::{benchmark::QueryBenchmark, perf::metric_kind::QueryMetricKind},
    ApiError,
};

pub mod data;
pub mod detector;
pub mod threshold;

pub struct Detectors {
    pub detectors: HashMap<MetricKindId, Detector>,
    pub benchmark_cache: HashMap<String, BenchmarkId>,
    pub metric_kind_cache: HashMap<String, MetricKindId>,
}

pub type BenchmarkId = i32;
pub type MetricKindId = i32;

impl Detectors {
    pub fn new(
        conn: &mut SqliteConnection,
        project_id: i32,
        branch_id: i32,
        testbed_id: i32,
        benchmarks: JsonBenchmarks,
    ) -> Result<Self, HttpError> {
        let metrics_map = JsonMetricsMap::from(benchmarks);

        // Create all benchmarks if they don't already exist
        let mut detectors = HashMap::with_capacity(metrics_map.inner.len());
        let mut benchmark_cache = HashMap::with_capacity(metrics_map.inner.len());
        let mut metric_kind_cache = HashMap::with_capacity(metrics_map.inner.len());
        for (benchmark_name, metrics_list) in metrics_map.inner {
            let benchmark_id = QueryBenchmark::get_or_create(conn, project_id, &benchmark_name)?;
            benchmark_cache.insert(benchmark_name, benchmark_id);

            // Create all metric kinds if they don't already exist
            // And create a detector for the branch/testbed/metric kind grouping if a threshold exists
            for metric_kind_key in metrics_list.inner.into_keys() {
                let metric_kind_id =
                    QueryMetricKind::get_or_create(conn, project_id, &metric_kind_key)?;
                metric_kind_cache.insert(metric_kind_key, metric_kind_id);

                if let Some(detector) = Detector::new(conn, branch_id, testbed_id, metric_kind_id)?
                {
                    detectors.insert(metric_kind_id, detector);
                }
            }
        }

        Ok(Self {
            detectors,
            benchmark_cache,
            metric_kind_cache,
        })
    }

    pub fn detect(
        &self,
        conn: &mut SqliteConnection,
        perf_id: i32,
        benchmark_id: i32,
        metric_kind_id: i32,
        metric: JsonMetric,
    ) -> Result<(), ApiError> {
        if let Some(detector) = self.detectors.get(&metric_kind_id) {
            detector.detect(conn, perf_id, benchmark_id, metric.value.into())
        } else {
            Ok(())
        }
    }
}
