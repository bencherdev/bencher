use bencher_json::report::{
    new::{
        JsonBenchmarks,
        JsonMetrics,
    },
    JsonMetricsMap,
};
use diesel::SqliteConnection;
use dropshot::HttpError;

use crate::db::model::threshold::PerfKind;
pub use self::threshold::Threshold;
use self::{
    latency::Latency,
    min_max_avg::MinMaxAvg,
    throughput::Throughput,
};
use crate::db::model::{
    benchmark::QueryBenchmark,
};

pub mod latency;
pub mod min_max_avg;
pub mod threshold;
pub mod throughput;

pub struct Thresholds {
    pub latency:    Option<Latency>,
    pub throughput: Option<Throughput>,
    pub compute:    Option<MinMaxAvg>,
    pub memory:     Option<MinMaxAvg>,
    pub storage:    Option<MinMaxAvg>,
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
            latency:    Latency::new(
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
            if let Some(latency) = &self.latency {
                latency.z_score(conn, perf_id, benchmark_name, json)?
            }
        }
        if let Some(json) = json_metrics.throughput {
            if let Some(throughput) = &self.throughput {
                // throughput.z_score(conn, perf_id, benchmark_name,
                // json)
            }
        }
        if let Some(json) = json_metrics.compute {
            if let Some(compute) = &self.compute {
                // compute.z_score(conn, perf_id, benchmark_name,
                // json)
            }
        }
        if let Some(json) = json_metrics.memory {
            if let Some(memory) = &self.memory {
                // memory.z_score(conn, perf_id, benchmark_name,
                // json)
            }
        }
        if let Some(json) = json_metrics.storage {
            if let Some(storage) = &self.storage {
                // storage.z_score(conn, perf_id, benchmark_name,
                // json)
            }
        }

        Ok(())
    }
}
