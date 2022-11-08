use bencher_json::project::report::new::{JsonBenchmarks, JsonMetrics};
use diesel::{RunQueryDsl, SqliteConnection};
use dropshot::HttpError;

use crate::{
    error::api_error,
    model::project::{
        benchmark::QueryBenchmark,
        perf::{InsertPerf, QueryPerf},
    },
    schema,
};

pub mod detectors;

use self::detectors::{detector::Detector, Detectors};

use super::{metric::InsertMetric, metric_kind::QueryMetricKind};

pub struct Metrics {
    pub project_id: i32,
    pub branch_id: i32,
    pub testbed_id: i32,
    pub report_id: i32,
    pub detectors: Detectors,
}

impl Metrics {
    pub fn new(
        conn: &mut SqliteConnection,
        project_id: i32,
        branch_id: i32,
        testbed_id: i32,
        report_id: i32,
        benchmarks: JsonBenchmarks,
    ) -> Result<Self, HttpError> {
        Ok(Self {
            project_id,
            branch_id,
            testbed_id,
            report_id,
            detectors: Detectors::new(conn, project_id, branch_id, testbed_id, benchmarks)?,
        })
    }

    pub fn insert(
        &mut self,
        conn: &mut SqliteConnection,
        iteration: usize,
        benchmark_name: String,
        json_metrics: JsonMetrics,
    ) -> Result<(), HttpError> {
        let benchmark_id = QueryBenchmark::get_or_create(conn, self.project_id, &benchmark_name)?;

        let insert_perf = InsertPerf::from_json(self.report_id, iteration, benchmark_id);
        diesel::insert_into(schema::perf::table)
            .values(&insert_perf)
            .execute(conn)
            .map_err(api_error!())?;
        let perf_id = QueryPerf::get_id(conn, &insert_perf.uuid)?;

        for (metric_kind_key, metric) in json_metrics.inner {
            let metric_kind_id =
                QueryMetricKind::get_or_create(conn, self.project_id, &metric_kind_key)?;

            let insert_metric = InsertMetric::from_json(perf_id, metric_kind_id, metric);
            diesel::insert_into(schema::metric::table)
                .values(&insert_metric)
                .execute(conn)
                .map_err(api_error!())?;

            if let Some(detector) =
                Detector::new(conn, self.branch_id, self.testbed_id, metric_kind_id)?
            {
                detector.detect(conn, perf_id, benchmark_id, metric)?;
            }
        }

        Ok(())
    }
}
