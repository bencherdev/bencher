use std::collections::HashMap;

use bencher_adapter::{
    results::adapter_metrics::AdapterMetrics, AdapterResults, AdapterResultsArray,
    Settings as AdapterSettings,
};
use bencher_json::{
    project::report::{Adapter, Iteration, JsonReportSettings},
    BenchmarkName, MeasureNameId,
};
use diesel::RunQueryDsl;
use dropshot::HttpError;
use http::StatusCode;
use slog::Logger;

use crate::{
    context::DbConnection,
    error::{bad_request_error, issue_error, resource_conflict_err},
    model::project::{
        benchmark::{BenchmarkId, QueryBenchmark},
        branch::BranchId,
        measure::{MeasureId, QueryMeasure},
        metric::{InsertMetric, QueryMetric},
        perf::{InsertPerf, QueryPerf},
        testbed::TestbedId,
        ProjectId,
    },
    schema,
};

pub mod detector;

use detector::Detector;

use super::ReportId;

/// `ReportResults` is used to process the report results.
pub struct ReportResults {
    pub project_id: ProjectId,
    pub branch_id: BranchId,
    pub testbed_id: TestbedId,
    pub report_id: ReportId,
    pub measure_cache: HashMap<MeasureNameId, MeasureId>,
    pub detector_cache: HashMap<MeasureId, Option<Detector>>,
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
            measure_cache: HashMap::new(),
            detector_cache: HashMap::new(),
        }
    }

    pub fn process(
        &mut self,
        log: &Logger,
        conn: &mut DbConnection,
        results_array: &[&str],
        adapter: Adapter,
        settings: JsonReportSettings,
        #[cfg(feature = "plus")] usage: &mut u32,
    ) -> Result<(), HttpError> {
        let adapter_settings = AdapterSettings::new(settings.average);
        let results_array = AdapterResultsArray::new(results_array, adapter, adapter_settings)
            .map_err(|e| {
                bad_request_error(format!(
                    "Failed to convert results with adapter ({adapter} | {settings:?}): {e}"
                ))
            })?;

        if let Some(fold) = settings.fold {
            let results = results_array.fold(fold);
            self.results(
                log,
                conn,
                Iteration::default(),
                results,
                #[cfg(feature = "plus")]
                usage,
            )?;
        } else {
            for (iteration, results) in results_array.inner.into_iter().enumerate() {
                self.results(
                    log,
                    conn,
                    iteration.into(),
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
        iteration: Iteration,
        results: AdapterResults,
        #[cfg(feature = "plus")] usage: &mut u32,
    ) -> Result<(), HttpError> {
        for (benchmark_name, metrics) in results.inner {
            self.metrics(
                log,
                conn,
                iteration,
                &benchmark_name,
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
        iteration: Iteration,
        benchmark_name: &BenchmarkName,
        metrics: AdapterMetrics,
        #[cfg(feature = "plus")] usage: &mut u32,
    ) -> Result<(), HttpError> {
        // If benchmark name is ignored then strip the special suffix before querying
        let (benchmark_name, ignore_benchmark) = benchmark_name.to_strip_ignore();
        let benchmark_id = self.benchmark_id(conn, benchmark_name)?;

        let insert_perf = InsertPerf::from_json(self.report_id, iteration, benchmark_id);
        diesel::insert_into(schema::perf::table)
            .values(&insert_perf)
            .execute(conn)
            .map_err(resource_conflict_err!(Perf, insert_perf))?;
        let perf_id = QueryPerf::get_id(conn, insert_perf.uuid)?;

        for (measure_key, metric) in metrics.inner {
            let measure_id = self.measure_id(conn, measure_key)?;

            let insert_metric = InsertMetric::from_json(perf_id, measure_id, metric);
            diesel::insert_into(schema::metric::table)
                .values(&insert_metric)
                .execute(conn)
                .map_err(resource_conflict_err!(Metric, insert_metric))?;

            #[cfg(feature = "plus")]
            {
                // Increment usage count
                *usage += 1;
            }

            // Ignored benchmarks do not get checked against the threshold even if one exists
            if !ignore_benchmark {
                if let Some(detector) = self.detector(conn, measure_id) {
                    let query_metric = QueryMetric::from_uuid(conn, insert_metric.uuid).map_err(|e| {
                        issue_error(
                            StatusCode::NOT_FOUND,
                            "Failed to find metric",
                            &format!("Failed to find new metric ({insert_metric:?}) for perf ({insert_perf:?}) even though it was just created on Bencher."),
                            e,
                        )
                    })?;
                    detector.detect(log, conn, benchmark_id, &query_metric)?;
                }
            }
        }

        Ok(())
    }

    fn benchmark_id(
        &mut self,
        conn: &mut DbConnection,
        benchmark_name: BenchmarkName,
    ) -> Result<BenchmarkId, HttpError> {
        QueryBenchmark::get_or_create(conn, self.project_id, benchmark_name).map_err(Into::into)
    }

    fn measure_id(
        &mut self,
        conn: &mut DbConnection,
        measure: MeasureNameId,
    ) -> Result<MeasureId, HttpError> {
        Ok(if let Some(id) = self.measure_cache.get(&measure) {
            *id
        } else {
            let measure_id = QueryMeasure::get_or_create(conn, self.project_id, &measure)?;
            self.measure_cache.insert(measure, measure_id);
            measure_id
        })
    }

    fn detector(&mut self, conn: &mut DbConnection, measure_id: MeasureId) -> Option<Detector> {
        if let Some(detector) = self.detector_cache.get(&measure_id) {
            detector.clone()
        } else {
            let detector = Detector::new(conn, self.branch_id, self.testbed_id, measure_id);
            self.detector_cache.insert(measure_id, detector.clone());
            detector
        }
    }
}
