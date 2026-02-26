use std::collections::HashMap;

use bencher_adapter::{
    AdapterResults, AdapterResultsArray, Settings as AdapterSettings,
    results::adapter_metrics::AdapterMetrics,
};
use bencher_json::{
    BenchmarkName, BenchmarkNameId, JsonNewMetric, MeasureNameId, Slug,
    project::report::{Adapter, Iteration, JsonReportSettings},
};
use diesel::{Connection as _, RunQueryDsl as _};
use dropshot::HttpError;
use slog::Logger;

use crate::macros::sql::last_insert_rowid;
use crate::model::spec::SpecId;
use crate::{
    auth_conn,
    context::ApiContext,
    error::bad_request_error,
    model::project::{
        ProjectId,
        benchmark::{BenchmarkId, QueryBenchmark},
        branch::{BranchId, head::HeadId},
        measure::{MeasureId, QueryMeasure},
        metric::InsertMetric,
        report::report_benchmark::{InsertReportBenchmark, ReportBenchmarkId},
        testbed::TestbedId,
    },
    schema, write_conn,
};

pub mod detector;

use detector::{Detector, PreparedDetection};

use super::ReportId;

/// `ReportResults` is used to process the report results.
pub struct ReportResults {
    pub project_id: ProjectId,
    pub branch_id: BranchId,
    pub head_id: HeadId,
    pub testbed_id: TestbedId,
    pub spec_id: Option<SpecId>,
    pub report_id: ReportId,
    pub benchmark_cache: HashMap<BenchmarkNameId, BenchmarkId>,
    pub measure_cache: HashMap<MeasureNameId, MeasureId>,
    pub detector_cache: HashMap<MeasureId, Option<Detector>>,
}

impl ReportResults {
    pub fn new(
        project_id: ProjectId,
        branch_id: BranchId,
        head_id: HeadId,
        testbed_id: TestbedId,
        spec_id: Option<SpecId>,
        report_id: ReportId,
    ) -> Self {
        Self {
            project_id,
            branch_id,
            head_id,
            testbed_id,
            spec_id,
            report_id,
            benchmark_cache: HashMap::new(),
            measure_cache: HashMap::new(),
            detector_cache: HashMap::new(),
        }
    }

    pub async fn process(
        &mut self,
        log: &Logger,
        context: &ApiContext,
        results_array: &[&str],
        adapter: Adapter,
        settings: JsonReportSettings,
        #[cfg(feature = "plus")] usage: &mut u32,
    ) -> Result<(), HttpError> {
        let adapter_settings = AdapterSettings::new(settings.average);
        let results_array = AdapterResultsArray::new(results_array, adapter, adapter_settings)
            .map_err(|e| {
                bad_request_error(format!(
                    "Failed to convert results with adapter ({adapter} | {settings:?}): {e}\n\nAre you sure {adapter} is the right adapter?\nRead more about adapters here: https://bencher.dev/docs/explanation/adapters/"
                ))
            })?;

        if let Some(fold) = settings.fold {
            let results = results_array.fold(fold);
            self.results(
                log,
                context,
                Iteration::default(),
                results,
                #[cfg(feature = "plus")]
                usage,
            )
            .await?;
        } else {
            for (iteration, results) in results_array.inner.into_iter().enumerate() {
                self.results(
                    log,
                    context,
                    iteration.into(),
                    results,
                    #[cfg(feature = "plus")]
                    usage,
                )
                .await?;
            }
        }

        Ok(())
    }

    async fn results(
        &mut self,
        log: &Logger,
        context: &ApiContext,
        iteration: Iteration,
        results: AdapterResults,
        #[cfg(feature = "plus")] usage: &mut u32,
    ) -> Result<(), HttpError> {
        // Phase 1: Pre-compute all data using read connections.
        // Resolve IDs (get_or_create), fetch historical data, compute boundaries.
        let mut prepared_benchmarks = Vec::with_capacity(results.inner.len());
        #[cfg(feature = "plus")]
        let mut metric_count: u32 = 0;

        for (benchmark, metrics) in results.inner {
            let prepared = self
                .prepare_benchmark(log, context, iteration, benchmark, metrics)
                .await?;
            #[cfg(feature = "plus")]
            {
                metric_count = metric_count
                    .saturating_add(u32::try_from(prepared.metrics.len()).unwrap_or(u32::MAX));
            }
            prepared_benchmarks.push(prepared);
        }

        // Phase 2: Write all data in a single transaction.
        let conn = write_conn!(context);
        conn.transaction(|conn| {
            for prepared in prepared_benchmarks {
                // Insert report_benchmark
                diesel::insert_into(schema::report_benchmark::table)
                    .values(&prepared.insert_report_benchmark)
                    .execute(conn)?;
                let report_benchmark_id: ReportBenchmarkId =
                    diesel::select(last_insert_rowid()).get_result(conn)?;

                // Insert all metrics for this benchmark
                for prepared_metric in prepared.metrics {
                    let insert_metric = InsertMetric::from_json(
                        report_benchmark_id,
                        prepared_metric.measure_id,
                        prepared_metric.metric,
                    );
                    diesel::insert_into(schema::metric::table)
                        .values(&insert_metric)
                        .execute(conn)?;

                    // If there's a prepared detection, write boundary + optional alert
                    if let Some(prepared_detection) = prepared_metric.detection {
                        let metric_id = diesel::select(last_insert_rowid()).get_result(conn)?;
                        prepared_detection.write(conn, metric_id)?;
                    }
                }
            }
            Ok::<_, diesel::result::Error>(())
        })
        .map_err(|e| {
            crate::error::issue_error(
                "Failed to write report results",
                "Failed to write report results in batch transaction:",
                e,
            )
        })?;

        #[cfg(feature = "plus")]
        {
            *usage = usage.saturating_add(metric_count);
        }

        Ok(())
    }

    /// Phase 1: Prepare all data for a single benchmark (reads + compute only).
    async fn prepare_benchmark(
        &mut self,
        log: &Logger,
        context: &ApiContext,
        iteration: Iteration,
        benchmark: BenchmarkNameId,
        metrics: AdapterMetrics,
    ) -> Result<PreparedBenchmark, HttpError> {
        // If benchmark name is ignored then strip the special suffix before querying
        let (benchmark, ignore_benchmark) = strip_ignore_suffix(benchmark);
        let benchmark_id = self.benchmark_id(context, benchmark).await?;

        let insert_report_benchmark =
            InsertReportBenchmark::from_json(self.report_id, iteration, benchmark_id);

        let mut prepared_metrics = Vec::with_capacity(metrics.inner.len());
        for (measure_key, metric) in metrics.inner {
            let measure_id = self.measure_id(context, measure_key).await?;

            // Pre-compute detection if a detector exists for this measure
            let detection = if let Some(detector) = self.detector(context, measure_id).await? {
                Some(detector.prepare_detection(
                    log,
                    auth_conn!(context),
                    benchmark_id,
                    metric.value.into(),
                    ignore_benchmark,
                )?)
            } else {
                None
            };

            prepared_metrics.push(PreparedMetric {
                measure_id,
                metric,
                detection,
            });
        }

        Ok(PreparedBenchmark {
            insert_report_benchmark,
            metrics: prepared_metrics,
        })
    }

    async fn benchmark_id(
        &mut self,
        context: &ApiContext,
        benchmark: BenchmarkNameId,
    ) -> Result<BenchmarkId, HttpError> {
        Ok(if let Some(id) = self.benchmark_cache.get(&benchmark) {
            *id
        } else {
            let benchmark_id =
                QueryBenchmark::get_or_create(context, self.project_id, &benchmark).await?;
            self.benchmark_cache.insert(benchmark, benchmark_id);
            benchmark_id
        })
    }

    async fn measure_id(
        &mut self,
        context: &ApiContext,
        measure: MeasureNameId,
    ) -> Result<MeasureId, HttpError> {
        Ok(if let Some(id) = self.measure_cache.get(&measure) {
            *id
        } else {
            let measure_id =
                QueryMeasure::get_or_create(context, self.project_id, &measure).await?;
            self.measure_cache.insert(measure, measure_id);
            measure_id
        })
    }

    async fn detector(
        &mut self,
        context: &ApiContext,
        measure_id: MeasureId,
    ) -> Result<Option<Detector>, HttpError> {
        Ok(
            if let Some(detector) = self.detector_cache.get(&measure_id) {
                detector.clone()
            } else {
                let detector = Detector::new(
                    auth_conn!(context),
                    self.branch_id,
                    self.head_id,
                    self.testbed_id,
                    self.spec_id,
                    measure_id,
                );
                self.detector_cache.insert(measure_id, detector.clone());
                detector
            },
        )
    }
}

/// Pre-computed data for a single benchmark within a report iteration.
struct PreparedBenchmark {
    insert_report_benchmark: InsertReportBenchmark,
    metrics: Vec<PreparedMetric>,
}

/// Pre-computed data for a single metric within a benchmark.
struct PreparedMetric {
    measure_id: MeasureId,
    metric: JsonNewMetric,
    detection: Option<PreparedDetection>,
}

fn strip_ignore_suffix(benchmark: BenchmarkNameId) -> (BenchmarkNameId, bool) {
    match benchmark {
        BenchmarkNameId::Uuid(uuid) => (BenchmarkNameId::Uuid(uuid), false),
        BenchmarkNameId::Slug(slug) => {
            // If the benchmark name ends with `-bencher-ignore`, strip the suffix and mark as ignored.
            // This value will be considered a name and not a slug for backwards compatibility.
            let slug_name = BenchmarkName::from(Slug::from(slug.clone()));
            let (name, is_ignored) = slug_name.strip_ignore();
            (
                if is_ignored {
                    BenchmarkNameId::Name(name)
                } else {
                    BenchmarkNameId::Slug(slug)
                },
                is_ignored,
            )
        },
        BenchmarkNameId::Name(name) => {
            let (name, is_ignored) = name.strip_ignore();
            (BenchmarkNameId::Name(name), is_ignored)
        },
    }
}

#[cfg(test)]
mod tests {
    use super::strip_ignore_suffix;
    use bencher_json::BenchmarkNameId;

    #[test]
    fn strip_ignore_suffix_with_suffix() {
        let name: BenchmarkNameId = "my-bench-bencher-ignore".parse().unwrap();
        let (stripped, is_ignored) = strip_ignore_suffix(name);
        assert!(is_ignored);
        assert_eq!(stripped.to_string(), "my-bench");
    }

    #[test]
    fn strip_ignore_suffix_without_suffix() {
        let name: BenchmarkNameId = "my-bench".parse().unwrap();
        let (original, is_ignored) = strip_ignore_suffix(name);
        assert!(!is_ignored);
        assert_eq!(original.to_string(), "my-bench");
    }
}
