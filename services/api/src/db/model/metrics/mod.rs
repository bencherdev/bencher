use bencher_json::report::new::{
    JsonBenchmarks,
    JsonMetrics,
};
use diesel::{
    RunQueryDsl,
    SqliteConnection,
};
use dropshot::HttpError;

use crate::{
    db::{
        model::{
            benchmark::QueryBenchmark,
            perf::{
                InsertPerf,
                QueryPerf,
            },
        },
        schema,
    },
    util::http_error,
};

pub mod data;
pub mod thresholds;

use self::thresholds::Thresholds;

const PERF_ERROR: &str = "Failed to create perf metrics.";

pub struct Metrics {
    pub project_id: i32,
    pub report_id:  i32,
    pub thresholds: Thresholds,
}

impl Metrics {
    pub fn new(
        conn: &SqliteConnection,
        project_id: i32,
        branch_id: i32,
        testbed_id: i32,
        report_id: i32,
        benchmarks: JsonBenchmarks,
    ) -> Result<Self, HttpError> {
        Ok(Self {
            project_id,
            report_id,
            thresholds: Thresholds::new(
                conn, project_id, branch_id, testbed_id, report_id, benchmarks,
            )?,
        })
    }

    pub fn benchmark(
        &mut self,
        conn: &SqliteConnection,
        iteration: i32,
        benchmark_name: &str,
        json_metrics: JsonMetrics,
    ) -> Result<(), HttpError> {
        // All benchmarks should already exist
        let benchmark_id = QueryBenchmark::get_id_from_name(conn, self.project_id, benchmark_name)?;

        let insert_perf =
            InsertPerf::from_json(conn, self.report_id, iteration, benchmark_id, json_metrics)?;

        diesel::insert_into(schema::perf::table)
            .values(&insert_perf)
            .execute(conn)
            .map_err(|_| http_error!(PERF_ERROR))?;

        let perf_id = QueryPerf::get_id(conn, &insert_perf.uuid)?;

        self.thresholds
            .z_test(conn, perf_id, benchmark_name, json_metrics)
    }
}
