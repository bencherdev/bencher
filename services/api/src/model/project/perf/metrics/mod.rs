use std::collections::HashMap;

use bencher_json::project::report::new::JsonMetrics;
use diesel::{RunQueryDsl, SqliteConnection};
use dropshot::HttpError;

use crate::{
    error::api_error,
    model::project::{
        benchmark::QueryBenchmark,
        perf::{InsertPerf, QueryPerf},
    },
    schema, ApiError,
};

pub mod detector;

use self::detector::Detector;

use super::{metric::InsertMetric, metric_kind::QueryMetricKind};

/// Metrics is used to add benchmarks, perf, metric kinds, metrics, and alerts.
pub struct Metrics {
    pub project_id: i32,
    pub branch_id: i32,
    pub testbed_id: i32,
    pub report_id: i32,
    pub benchmark_cache: HashMap<String, i32>,
    pub metric_kind_cache: HashMap<String, i32>,
    pub detector_cache: HashMap<i32, Option<Detector>>,
}

impl Metrics {
    pub fn new(
        project_id: i32,
        branch_id: i32,
        testbed_id: i32,
        report_id: i32,
    ) -> Result<Self, HttpError> {
        Ok(Self {
            project_id,
            branch_id,
            testbed_id,
            report_id,
            benchmark_cache: HashMap::new(),
            metric_kind_cache: HashMap::new(),
            detector_cache: HashMap::new(),
        })
    }

    pub fn insert(
        &mut self,
        conn: &mut SqliteConnection,
        iteration: usize,
        benchmark_name: String,
        json_metrics: JsonMetrics,
    ) -> Result<(), HttpError> {
        let benchmark_id = self.benchmark_id(conn, benchmark_name)?;

        let insert_perf = InsertPerf::from_json(self.report_id, iteration, benchmark_id);
        diesel::insert_into(schema::perf::table)
            .values(&insert_perf)
            .execute(conn)
            .map_err(api_error!())?;
        let perf_id = QueryPerf::get_id(conn, &insert_perf.uuid)?;

        for (metric_kind_key, metric) in json_metrics.inner {
            let metric_kind_id = self.metric_kind_id(conn, metric_kind_key)?;

            let insert_metric = InsertMetric::from_json(perf_id, metric_kind_id, metric);
            diesel::insert_into(schema::metric::table)
                .values(&insert_metric)
                .execute(conn)
                .map_err(api_error!())?;

            if let Some(detector) = self.detector(conn, metric_kind_id)? {
                detector.detect(conn, perf_id, benchmark_id, metric)?;
            }
        }

        Ok(())
    }

    fn benchmark_id(
        &mut self,
        conn: &mut SqliteConnection,
        benchmark_name: String,
    ) -> Result<i32, ApiError> {
        Ok(
            if let Some(id) = self.benchmark_cache.get(&benchmark_name) {
                *id
            } else {
                let benchmark_id =
                    QueryBenchmark::get_or_create(conn, self.project_id, &benchmark_name)?;
                self.benchmark_cache.insert(benchmark_name, benchmark_id);
                benchmark_id
            },
        )
    }

    fn metric_kind_id(
        &mut self,
        conn: &mut SqliteConnection,
        metric_kind_key: String,
    ) -> Result<i32, ApiError> {
        Ok(
            if let Some(id) = self.metric_kind_cache.get(&metric_kind_key) {
                *id
            } else {
                let metric_kind_id =
                    QueryMetricKind::get_or_create(conn, self.project_id, &metric_kind_key)?;
                self.metric_kind_cache
                    .insert(metric_kind_key, metric_kind_id);
                metric_kind_id
            },
        )
    }

    fn detector(
        &mut self,
        conn: &mut SqliteConnection,
        metric_kind_id: i32,
    ) -> Result<Option<Detector>, ApiError> {
        Ok(
            if let Some(detector) = self.detector_cache.get(&metric_kind_id) {
                detector.clone()
            } else {
                let detector =
                    Detector::new(conn, self.branch_id, self.testbed_id, metric_kind_id)?;
                self.detector_cache.insert(metric_kind_id, detector.clone());
                detector
            },
        )
    }
}
