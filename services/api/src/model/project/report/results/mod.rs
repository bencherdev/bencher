use std::collections::HashMap;

use bencher_adapter::{
    results::{adapter_metrics::AdapterMetrics, MetricKind},
    AdapterResults, AdapterResultsArray, Settings as AdapterSettings,
};
use bencher_json::{
    project::report::{JsonAdapter, JsonReportSettings},
    BenchmarkName,
};
use diesel::RunQueryDsl;
use slog::Logger;

use crate::{
    context::DbConnection,
    error::api_error,
    model::project::{
        benchmark::{BenchmarkId, QueryBenchmark},
        branch::BranchId,
        metric::{InsertMetric, QueryMetric},
        metric_kind::{MetricKindId, QueryMetricKind},
        perf::{InsertPerf, QueryPerf},
        testbed::TestbedId,
        ProjectId,
    },
    schema, ApiError,
};

pub mod detector;

use detector::Detector;

use super::ReportId;

/// `ReportResults` is used to add benchmarks, perf, metric kinds, metrics, and alerts.
pub struct ReportResults {
    pub project_id: ProjectId,
    pub branch_id: BranchId,
    pub testbed_id: TestbedId,
    pub report_id: ReportId,
    pub metric_kind_cache: HashMap<MetricKind, MetricKindId>,
    pub detector_cache: HashMap<MetricKindId, Option<Detector>>,
}

impl ReportResults {
    pub fn new(
        project_id: ProjectId,
        branch_id: BranchId,
        testbed_id: TestbedId,
        report_id: ReportId,
    ) -> Self {
        Self {
            project_id,
            branch_id,
            testbed_id,
            report_id,
            metric_kind_cache: HashMap::new(),
            detector_cache: HashMap::new(),
        }
    }

    pub fn process(
        &mut self,
        log: &Logger,
        conn: &mut DbConnection,
        results_array: &[&str],
        adapter: JsonAdapter,
        settings: JsonReportSettings,
        #[cfg(feature = "plus")] usage: &mut u64,
    ) -> Result<(), ApiError> {
        let adapter_settings = AdapterSettings::new(settings.average);
        let results_array = AdapterResultsArray::new(results_array, adapter, adapter_settings)?;

        if let Some(fold) = settings.fold {
            let results = results_array.fold(fold);
            self.results(
                log,
                conn,
                0,
                results,
                #[cfg(feature = "plus")]
                usage,
            )?;
        } else {
            for (iteration, results) in results_array.inner.into_iter().enumerate() {
                self.results(
                    log,
                    conn,
                    iteration,
                    results,
                    #[cfg(feature = "plus")]
                    usage,
                )?;
            }
        };

        Ok(())
    }

    fn results(
        &mut self,
        log: &Logger,
        conn: &mut DbConnection,
        iteration: usize,
        results: AdapterResults,
        #[cfg(feature = "plus")] usage: &mut u64,
    ) -> Result<(), ApiError> {
        for (benchmark_name, metrics) in results.inner {
            self.metrics(
                log,
                conn,
                iteration,
                benchmark_name,
                metrics,
                #[cfg(feature = "plus")]
                usage,
            )?;
        }
        Ok(())
    }

    fn metrics(
        &mut self,
        log: &Logger,
        conn: &mut DbConnection,
        iteration: usize,
        benchmark_name: BenchmarkName,
        metrics: AdapterMetrics,
        #[cfg(feature = "plus")] usage: &mut u64,
    ) -> Result<(), ApiError> {
        // If benchmark name is ignored then strip the special suffix before querying
        let (benchmark_name, ignore_benchmark) = benchmark_name.strip_ignore();
        let benchmark_id = self.benchmark_id(conn, benchmark_name)?;

        let insert_perf = InsertPerf::from_json(self.report_id, iteration, benchmark_id);
        diesel::insert_into(schema::perf::table)
            .values(&insert_perf)
            .execute(conn)
            .map_err(api_error!())?;
        let perf_id = QueryPerf::get_id(conn, &insert_perf.uuid)?;

        for (metric_kind_key, metric) in metrics.inner {
            let metric_kind_id = self.metric_kind_id(conn, metric_kind_key)?;

            let insert_metric = InsertMetric::from_json(perf_id, metric_kind_id, metric);
            diesel::insert_into(schema::metric::table)
                .values(&insert_metric)
                .execute(conn)
                .map_err(api_error!())?;

            #[cfg(feature = "plus")]
            {
                // Increment usage count
                *usage += 1;
            }

            // Ignored benchmarks do not get checked against the threshold even if one exists
            if !ignore_benchmark {
                if let Some(detector) = self.detector(conn, metric_kind_id)? {
                    let query_metric = QueryMetric::from_uuid(conn, insert_metric.uuid)?;
                    detector.detect(log, conn, benchmark_id, query_metric)?;
                }
            }
        }

        Ok(())
    }

    fn benchmark_id(
        &mut self,
        conn: &mut DbConnection,
        benchmark_name: &str,
    ) -> Result<BenchmarkId, ApiError> {
        QueryBenchmark::get_or_create(conn, self.project_id, benchmark_name)
    }

    fn metric_kind_id(
        &mut self,
        conn: &mut DbConnection,
        metric_kind_key: MetricKind,
    ) -> Result<MetricKindId, ApiError> {
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
        conn: &mut DbConnection,
        metric_kind_id: MetricKindId,
    ) -> Result<Option<Detector>, ApiError> {
        Ok(
            if let Some(detector) = self.detector_cache.get(&metric_kind_id) {
                detector.clone()
            } else {
                let detector =
                    Detector::new(conn, metric_kind_id, self.branch_id, self.testbed_id)?;
                self.detector_cache.insert(metric_kind_id, detector.clone());
                detector
            },
        )
    }
}
