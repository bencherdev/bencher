use bencher_json::report::{
    new::{JsonBenchmarks, JsonMetrics},
    JsonMetricsMap,
};
use diesel::SqliteConnection;
use dropshot::HttpError;

use self::detector::Detector;
pub use self::threshold::Threshold;
use crate::db::model::{benchmark::QueryBenchmark, threshold::PerfKind};

pub mod detector;
pub mod threshold;

pub struct Thresholds {
    pub latency: Option<Detector>,
    pub throughput: Option<Detector>,
    pub compute: Option<Detector>,
    pub memory: Option<Detector>,
    pub storage: Option<Detector>,
}

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
            latency: Detector::new(
                conn,
                branch_id,
                testbed_id,
                &benchmarks,
                &metrics_map,
                PerfKind::Latency,
            )?,
            throughput: None,
            compute: None,
            memory: None,
            storage: None,
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
