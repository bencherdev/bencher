use bencher_json::project::report::{
    metric_kind::JsonNewMetricKind,
    new::{JsonBenchmarks, JsonMetrics},
};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl, SqliteConnection};
use dropshot::HttpError;

use crate::{
    error::api_error,
    model::project::{
        benchmark::QueryBenchmark,
        perf::{InsertPerf, QueryPerf},
    },
    schema, ApiError,
};

pub mod data;
pub mod thresholds;

use self::thresholds::Thresholds;

use super::perf::{
    metric::InsertMetric,
    metric_kind::{InsertMetricKind, QueryMetricKind},
};

pub struct Metrics {
    pub project_id: i32,
    pub report_id: i32,
    pub thresholds: Thresholds,
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
            report_id,
            thresholds: Thresholds::new(conn, project_id, branch_id, testbed_id, benchmarks)?,
        })
    }

    pub fn benchmark(
        &mut self,
        conn: &mut SqliteConnection,
        project_id: i32,
        iteration: i32,
        benchmark_name: &str,
        json_metrics: JsonMetrics,
    ) -> Result<(), HttpError> {
        // All benchmarks should already exist
        let benchmark_id = QueryBenchmark::get_id_from_name(conn, self.project_id, benchmark_name)?;

        let insert_perf = InsertPerf::from_json(conn, self.report_id, iteration, benchmark_id)?;

        diesel::insert_into(schema::perf::table)
            .values(&insert_perf)
            .execute(conn)
            .map_err(api_error!())?;

        let perf_id = QueryPerf::get_id(conn, &insert_perf.uuid)?;

        for (metric_kind, metric) in &json_metrics.inner {
            let metric_kind_id = metric_kind_id(conn, project_id, metric_kind)?;
            let insert_metric = InsertMetric::from_json(perf_id, metric_kind_id, *metric);
            diesel::insert_into(schema::metric::table)
                .values(&insert_metric)
                .execute(conn)
                .map_err(api_error!())?;
            let metric_id = schema::metric::table
                .filter(schema::metric::uuid.eq(&insert_metric.uuid))
                .select(schema::metric::id)
                .first::<i32>(conn)
                .map_err(api_error!())?;
        }

        self.thresholds
            .test(conn, perf_id, benchmark_name, json_metrics)
    }
}

// If the metric kind does not already exist, then create one on the fly
fn metric_kind_id(
    conn: &mut SqliteConnection,
    project_id: i32,
    metric_kind: &str,
) -> Result<i32, ApiError> {
    if let Ok(resource_id) = metric_kind.parse() {
        if let Ok(metric_kind) = QueryMetricKind::from_resource_id(conn, &resource_id) {
            return Ok(metric_kind.id);
        }
    }

    let insert_metric_kind = InsertMetricKind::from_json_inner(
        conn,
        project_id,
        JsonNewMetricKind {
            name: metric_kind.into(),
            slug: None,
            units: None,
        },
    );

    diesel::insert_into(schema::metric_kind::table)
        .values(&insert_metric_kind)
        .execute(conn)
        .map_err(api_error!())?;

    QueryMetricKind::get_id(conn, insert_metric_kind.uuid)
}
