use bencher_json::report::{
    new::JsonBenchmarks,
    JsonMetricsMap,
};
use diesel::SqliteConnection;
use dropshot::HttpError;

pub use self::threshold::Threshold;
use self::{
    latency::Latency,
    min_max_avg::MinMaxAvg,
    throughput::Throughput,
};
use crate::db::model::benchmark::QueryBenchmark;

pub mod latency;
pub mod min_max_avg;
pub mod threshold;
pub mod throughput;

const PERF_ERROR: &str = "Failed to create perf statistic.";

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
            latency:    Latency::new(conn, branch_id, testbed_id, &benchmarks, &metrics_map)?,
            throughput: None,
            compute:    None,
            memory:     None,
            storage:    None,
        })
    }
}
