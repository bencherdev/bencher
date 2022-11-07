use std::collections::HashMap;

use bencher_json::project::report::{
    new::{JsonBenchmarks, JsonMetrics},
    JsonMetricsMap,
};
use diesel::SqliteConnection;
use dropshot::HttpError;

use self::detector::Detector;
pub use self::threshold::Threshold;
use crate::model::project::{benchmark::QueryBenchmark, perf::metric_kind::QueryMetricKind};

pub mod detector;
pub mod threshold;

pub struct Thresholds {
    pub detectors: HashMap<String, Detector>,
    pub benchmarks: HashMap<String, i32>,
}

pub type MetricKindId = i32;

impl Thresholds {
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
        let mut benchmarks = HashMap::with_capacity(metrics_map.inner.len());
        for (benchmark_name, metrics_list) in metrics_map.inner {
            let benchmark_id = QueryBenchmark::get_or_create(conn, project_id, &benchmark_name)?;
            benchmarks.insert(benchmark_name, benchmark_id);
            // Create all metric kinds if they don't already exist
            // And create a detector for the branch/testbed/metric kind grouping
            for metric_kind_key in metrics_list.inner.into_keys() {
                let metric_kind_id =
                    QueryMetricKind::get_or_create(conn, project_id, &metric_kind_key)?;
                if let Some(detector) = Detector::new(conn, branch_id, testbed_id, metric_kind_id)?
                {
                    detectors.insert(metric_kind_key, detector);
                }
            }
        }

        Ok(Self {
            detectors,
            benchmarks,
        })
    }

    pub fn test(
        &mut self,
        conn: &mut SqliteConnection,
        perf_id: i32,
        benchmark_name: &str,
        json_metrics: JsonMetrics,
    ) -> Result<(), HttpError> {
        if let Some(json) = json_metrics.latency {
            if let Some(detector) = &mut self.latency {
                detector.test(conn, perf_id, benchmark_name, json.duration as f64)?;
            }
        }
        if let Some(json) = json_metrics.throughput {
            if let Some(detector) = &mut self.throughput {
                detector.test(
                    conn,
                    perf_id,
                    benchmark_name,
                    json.per_unit_time(&json.events).into(),
                )?;
            }
        }
        if let Some(json) = json_metrics.compute {
            if let Some(detector) = &mut self.compute {
                detector.test(conn, perf_id, benchmark_name, json.avg.into())?;
            }
        }
        if let Some(json) = json_metrics.memory {
            if let Some(detector) = &mut self.memory {
                detector.test(conn, perf_id, benchmark_name, json.avg.into())?;
            }
        }
        if let Some(json) = json_metrics.storage {
            if let Some(detector) = &mut self.storage {
                detector.test(conn, perf_id, benchmark_name, json.avg.into())?;
            }
        }

        Ok(())
    }
}
