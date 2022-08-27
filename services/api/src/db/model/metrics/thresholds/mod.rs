use bencher_json::report::{
    new::{
        JsonBenchmarks,
        JsonMetrics,
    },
    JsonMetricsMap,
};
use diesel::SqliteConnection;
use dropshot::HttpError;

use self::detector::Detector;
pub use self::threshold::Threshold;
use crate::db::model::{
    benchmark::QueryBenchmark,
    threshold::PerfKind,
};

pub mod detector;
pub mod threshold;

pub struct Thresholds {
    pub latency:    Option<Detector>,
    pub throughput: Option<Detector>,
    pub compute:    Option<Detector>,
    pub memory:     Option<Detector>,
    pub storage:    Option<Detector>,
}

impl Thresholds {
    pub fn new(
        conn: &SqliteConnection,
        project_id: i32,
        branch_id: i32,
        testbed_id: i32,
        report_id: i32,
        benchmarks: JsonBenchmarks,
    ) -> Result<Self, HttpError> {
        let metrics_map = JsonMetricsMap::from(benchmarks);

        // Create all benchmarks if they don't already exist
        let benchmark_names: Vec<String> = metrics_map.inner.keys().cloned().collect();
        let mut benchmark_ids = Vec::with_capacity(benchmark_names.len());
        for name in &benchmark_names {
            benchmark_ids.push(QueryBenchmark::get_or_create(conn, project_id, name)?);
        }
        let benchmarks: Vec<(String, i32)> = benchmark_names
            .into_iter()
            .zip(benchmark_ids.into_iter())
            .collect();

        Ok(Self {
            latency:    Detector::new(
                conn,
                branch_id,
                testbed_id,
                report_id,
                &benchmarks,
                &metrics_map,
                PerfKind::Latency,
            )?,
            throughput: None,
            compute:    None,
            memory:     None,
            storage:    None,
        })
    }

    pub fn z_score(
        &self,
        conn: &SqliteConnection,
        perf_id: i32,
        benchmark_name: &str,
        json_metrics: JsonMetrics,
    ) -> Result<(), HttpError> {
        if let Some(json) = json_metrics.latency {
            if let Some(detector) = &self.latency {
                detector.z_score(conn, perf_id, benchmark_name, json.duration as f64)?
            }
        }
        if let Some(json) = json_metrics.throughput {
            if let Some(detector) = &self.throughput {
                // throughput.z_score(conn, perf_id, benchmark_name,
                // json)
            }
        }
        if let Some(json) = json_metrics.compute {
            if let Some(detector) = &self.compute {
                // compute.z_score(conn, perf_id, benchmark_name,
                // json)
            }
        }
        if let Some(json) = json_metrics.memory {
            if let Some(detector) = &self.memory {
                // memory.z_score(conn, perf_id, benchmark_name,
                // json)
            }
        }
        if let Some(json) = json_metrics.storage {
            if let Some(detector) = &self.storage {
                // storage.z_score(conn, perf_id, benchmark_name,
                // json)
            }
        }

        Ok(())
    }
}
