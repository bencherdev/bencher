use std::collections::HashMap;

use bencher_adapter::{
    AdapterError, AdapterResults, AdapterResultsArray, Settings as AdapterSettings,
    results::adapter_metrics::{AdapterMetrics, NamedMap},
};
use bencher_json::{
    BenchmarkName, BenchmarkNameId, BmfVersion, MeasureNameId, MetricName, ParameterSet, Slug,
    project::report::{Adapter, Iteration, JsonReportSettings},
};
use diesel::RunQueryDsl as _;
use dropshot::HttpError;
use slog::Logger;

#[cfg(feature = "plus")]
use bencher_json::DateTime;

use crate::macros::sql::last_insert_rowid;
use crate::model::spec::SpecId;
#[cfg(feature = "plus")]
use crate::model::{
    organization::OrganizationId,
    project::series::{SeriesKey, upsert_series_last_seen},
};
use crate::{
    auth_conn,
    context::ApiContext,
    error::{bad_request_error, issue_error},
    model::project::{
        ProjectId,
        benchmark::{BenchmarkId, QueryBenchmark},
        branch::{BranchId, head::HeadId},
        measure::{MeasureId, QueryMeasure},
        metric::InsertMetric,
        report::report_benchmark::{InsertReportBenchmark, ReportBenchmarkId},
        testbed::TestbedId,
        variant::{QueryVariant, VariantId},
    },
    schema, write_transaction,
};

pub mod detector;

use detector::{Detector, PreparedDetection, Threshold};

use super::ReportId;

/// `ReportResults` is used to process the report results.
pub struct ReportResults {
    pub project_id: ProjectId,
    pub branch_id: BranchId,
    pub head_id: HeadId,
    pub testbed_id: TestbedId,
    pub spec_id: Option<SpecId>,
    pub report_id: ReportId,
    // The owning organization and report end time written into the active-series cache
    // on ingest. Bundled so they travel together and the constructor stays within its
    // argument count.
    #[cfg(feature = "plus")]
    pub series_cache: SeriesCacheContext,
    pub benchmark_cache: HashMap<BenchmarkNameId, BenchmarkId>,
    pub variant_cache: HashMap<(BenchmarkId, ParameterSet), VariantId>,
    pub measure_cache: HashMap<MeasureNameId, MeasureId>,
    pub threshold_cache: HashMap<MeasureId, Vec<Threshold>>,
}

/// The report context the active-series cache write needs: the owning organization
/// (denormalized into each `series_last_seen` row so a billing read is a single index
/// scan) and the report's server-side creation time (written as each ingested series'
/// `last_seen`). Creation time, not the user-supplied `end_time`, is used so a report
/// cannot dodge active-series billing by claiming a far-future `end_time`.
#[cfg(feature = "plus")]
pub struct SeriesCacheContext {
    pub organization_id: OrganizationId,
    pub report_created: DateTime,
}

impl ReportResults {
    pub fn new(
        project_id: ProjectId,
        branch_id: BranchId,
        head_id: HeadId,
        testbed_id: TestbedId,
        spec_id: Option<SpecId>,
        report_id: ReportId,
        #[cfg(feature = "plus")] series_cache: SeriesCacheContext,
    ) -> Self {
        Self {
            project_id,
            branch_id,
            head_id,
            testbed_id,
            spec_id,
            report_id,
            #[cfg(feature = "plus")]
            series_cache,
            benchmark_cache: HashMap::new(),
            variant_cache: HashMap::new(),
            measure_cache: HashMap::new(),
            threshold_cache: HashMap::new(),
        }
    }

    /// Process report results by iterating over each result set sequentially.
    ///
    /// The sequential per-iteration processing is load-bearing:
    /// each iteration performs a Phase 1 read (querying historical metrics via `metrics_data()`)
    /// followed by a Phase 2 write (committing new metrics).
    /// Iteration N+1's boundary detection must see iteration N's committed metrics,
    /// so iterations cannot be collapsed into a single deferred transaction.
    #[expect(
        clippy::too_many_arguments,
        reason = "cfg features add extra params, as does the report payload version"
    )]
    pub async fn process(
        &mut self,
        log: &Logger,
        context: &ApiContext,
        results_array: &[&str],
        adapter: Adapter,
        settings: JsonReportSettings,
        bmf_version: BmfVersion,
        #[cfg(feature = "plus")] usage: &mut u32,
    ) -> Result<(), HttpError> {
        #[cfg(feature = "otel")]
        let process_start = context.clock.now();

        self.threshold_cache =
            Threshold::load(auth_conn!(context), self.branch_id, self.testbed_id).map_err(|e| {
                issue_error(
                    "Failed to load report thresholds",
                    "Failed to load the thresholds for a report:",
                    e,
                )
            })?;

        let adapter_settings = AdapterSettings::new(settings.average, bmf_version);
        let results_array = AdapterResultsArray::new(results_array, adapter, adapter_settings)
            .map_err(|e| match e {
                // The adapter is right and the declared version is wrong, so no adapter hint.
                AdapterError::BmfVersion { .. } => bad_request_error(e.to_string()),
                AdapterError::Valid(_) | AdapterError::BenchmarkUnits(_) | AdapterError::Convert(_) => {
                    bad_request_error(format!(
                        "Failed to convert results with adapter ({adapter} | {settings:?}): {e}\n\nAre you sure {adapter} is the right adapter?\nRead more about adapters here: https://bencher.dev/docs/explanation/adapters/"
                    ))
                },
            })?;

        // The per measure cap has already truncated, so this is the report of it and
        // never an ingest error: a harness that names more statistics than the cap
        // allows still gets its report. The adapter counts; the log line and the
        // counter are here because this is where the providers are in scope.
        let dropped_names = results_array.dropped_names();
        let report_id = self.report_id;
        if dropped_names > 0 {
            slog::warn!(
                log,
                "Dropped {dropped_names} named metric value(s) over the per measure cap for report ({report_id})"
            );
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment_by(
                bencher_otel::ApiCounter::MetricNamesDropped,
                u64::try_from(dropped_names).unwrap_or(u64::MAX),
            );
        }

        // Fold is a BMF v0 operation and nothing else: the mean of per iteration
        // `p99` values is not the `p99` of the pooled sample. A v1 payload with fold
        // requested warns and ingests unfolded, one `report_benchmark` row per
        // iteration, because a harness upgrade must never turn a pipeline red.
        let results_array = if let Some(fold) = settings.fold {
            match results_array.foldable() {
                Ok(foldable) => vec![foldable.fold(fold).into()],
                Err(unfoldable) => {
                    slog::warn!(
                        log,
                        "Ignoring the requested fold ({fold:?}) for report ({report_id}): fold is not supported for benchmark parameters or named metric values"
                    );
                    unfoldable.inner
                },
            }
        } else {
            results_array.inner
        };

        for (iteration, results) in results_array.into_iter().enumerate() {
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

        #[cfg(feature = "otel")]
        {
            let duration_secs = process_start.elapsed_secs(context.clock.now());
            bencher_otel::ApiMeter::record(
                bencher_otel::ApiHistogram::ReportProcessDuration,
                duration_secs,
            );
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
        let mut prepared_variants = Vec::with_capacity(results.inner.len());

        // A BMF v1 entry that names no measure measured nothing, so it writes
        // nothing: its variant is never resolved, it writes no
        // `report_benchmark` row, it touches no series, and it bills nothing.
        //
        // Gated on the version because BMF v0 says the same thing with `{"bench": {}}`
        // and has always written that row on the benchmark's empty variant.
        // Skipping every empty variant would move a v0 payload, which this whole
        // layer promises not to do.
        let skip_empty_variants = results.version == BmfVersion::V1;

        for (benchmark, entries) in results.inner {
            // If benchmark name is ignored then strip the special suffix before querying
            let (benchmark, ignore_benchmark) = strip_ignore_suffix(benchmark);
            let benchmark_id = self.benchmark_id(context, benchmark).await?;
            // A benchmark reports one result per variant, and
            // each is its own `report_benchmark` row with its own series history.
            for (parameters, metrics) in entries {
                if skip_empty_variants && metrics.inner.is_empty() {
                    continue;
                }
                let prepared = self
                    .prepare_variant(
                        log,
                        context,
                        iteration,
                        benchmark_id,
                        ignore_benchmark,
                        parameters,
                        metrics,
                    )
                    .await?;
                prepared_variants.push(prepared);
            }
        }

        // Compute metric count once before acquiring write lock. Metrics collapse
        // into their measure's point estimate, so this counts exactly the rows
        // `QueryMetric::usage` reads back: one per measure that named a `value`.
        let iteration_metric_count: i32 = prepared_variants
            .iter()
            .map(|variant| {
                i32::try_from(
                    variant
                        .measures
                        .iter()
                        .filter(|measure| measure.named.contains_key(&MetricName::value()))
                        .count(),
                )
                .unwrap_or(i32::MAX)
            })
            .fold(0i32, i32::saturating_add);

        // Phase 2: Write all data in a single transaction.
        #[cfg(feature = "otel")]
        let write_start = context.clock.now();

        write_transaction!(context, |conn| {
            // Series (testbed x benchmark x variant x measure) seen in this iteration,
            // upserted into the active-series cache in this same transaction so the
            // cache cannot drift from the metrics it bills.
            #[cfg(feature = "plus")]
            let mut series_keys: Vec<SeriesKey> = Vec::new();
            for prepared in prepared_variants {
                // The series this variant touches, taken before it is written out.
                #[cfg(feature = "plus")]
                let variant_series = prepared.series_keys(self.testbed_id);
                write_variant(conn, prepared)?;
                #[cfg(feature = "plus")]
                series_keys.extend(variant_series);
            }

            // Upsert metric count summary (count computed before acquiring write lock)
            super::upsert_metric_count(conn, self.report_id, iteration_metric_count)?;

            // Refresh each seen series' last_seen to this report's creation time, in the
            // same transaction as the metric inserts above. The series keys are distinct
            // within an iteration; repeats across iterations are idempotent.
            #[cfg(feature = "plus")]
            for series in series_keys {
                upsert_series_last_seen(
                    conn,
                    self.series_cache.organization_id,
                    self.project_id,
                    series,
                    self.series_cache.report_created,
                )?;
            }

            diesel::QueryResult::Ok(())
        })
        .map_err(|e| {
            issue_error(
                "Failed to write report results",
                "Failed to write report results in batch transaction:",
                e,
            )
        })?;

        #[cfg(feature = "otel")]
        {
            let duration_secs = write_start.elapsed_secs(context.clock.now());
            bencher_otel::ApiMeter::record(
                bencher_otel::ApiHistogram::ReportWriteDuration,
                duration_secs,
            );
        }

        #[cfg(feature = "plus")]
        {
            *usage =
                usage.saturating_add(u32::try_from(iteration_metric_count).unwrap_or(u32::MAX));
        }

        Ok(())
    }

    /// Phase 1: Prepare all data for a single variant (reads + compute only).
    #[expect(
        clippy::too_many_arguments,
        reason = "a variant is a benchmark, its parameters, and its metrics"
    )]
    async fn prepare_variant(
        &mut self,
        log: &Logger,
        context: &ApiContext,
        iteration: Iteration,
        benchmark_id: BenchmarkId,
        ignore_benchmark: bool,
        parameters: ParameterSet,
        metrics: AdapterMetrics,
    ) -> Result<PreparedVariant, HttpError> {
        // Resolved here in Phase 1, alongside the benchmark and the measures, so the
        // Phase 2 write transaction stays read free and never nests a transaction.
        let variant_id = self
            .variant_id(context, benchmark_id, parameters.clone())
            .await?;

        let insert_report_benchmark =
            InsertReportBenchmark::from_json(self.report_id, iteration, benchmark_id, variant_id);

        let mut prepared_measures = Vec::with_capacity(metrics.inner.len());
        for (measure_key, metric) in metrics.inner {
            let measure_id = self.measure_id(context, measure_key).await?;
            let named = metric.inner;

            let mut detections: HashMap<MetricName, Vec<PreparedDetection>> = HashMap::new();
            for threshold in self.threshold_cache.get(&measure_id).into_iter().flatten() {
                let Some(value) = named.get(&threshold.metric).copied() else {
                    continue;
                };
                if !threshold.checks(&parameters) {
                    continue;
                }
                let name = threshold.metric.clone();
                let detector = Detector {
                    head_id: self.head_id,
                    testbed_id: self.testbed_id,
                    spec_id: self.spec_id,
                    measure_id,
                    threshold: threshold.clone(),
                };
                let detection = detector.prepare_detection(
                    log,
                    auth_conn!(context),
                    benchmark_id,
                    variant_id,
                    value.into_inner(),
                    ignore_benchmark,
                )?;
                detections.entry(name).or_default().push(detection);
            }

            prepared_measures.push(PreparedMeasure {
                measure_id,
                named,
                detections,
            });
        }

        Ok(PreparedVariant {
            insert_report_benchmark,
            measures: prepared_measures,
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

    /// The variant's row, keyed on both the benchmark and its parameters:
    /// one benchmark has many variants.
    async fn variant_id(
        &mut self,
        context: &ApiContext,
        benchmark_id: BenchmarkId,
        parameters: ParameterSet,
    ) -> Result<VariantId, HttpError> {
        let key = (benchmark_id, parameters);
        Ok(if let Some(id) = self.variant_cache.get(&key) {
            *id
        } else {
            let variant_id =
                QueryVariant::get_or_create(context, self.project_id, benchmark_id, &key.1).await?;
            self.variant_cache.insert(key, variant_id);
            variant_id
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
}

/// Pre-computed data for a single variant within a report iteration.
struct PreparedVariant {
    insert_report_benchmark: InsertReportBenchmark,
    measures: Vec<PreparedMeasure>,
}

impl PreparedVariant {
    /// The series this variant bills: one per measure, however many names the
    /// measure carried.
    #[cfg(feature = "plus")]
    fn series_keys(&self, testbed_id: TestbedId) -> Vec<SeriesKey> {
        self.measures
            .iter()
            .map(|prepared_measure| SeriesKey {
                testbed_id,
                benchmark_id: self.insert_report_benchmark.benchmark_id,
                variant_id: self.insert_report_benchmark.variant_id,
                measure_id: prepared_measure.measure_id,
            })
            .collect()
    }
}

/// Phase 2: write one variant's `report_benchmark` row, every named metric row
/// under it, and the boundary and alert its point estimate earned.
///
/// Runs inside the ingest write transaction and opens none of its own.
fn write_variant(
    conn: &mut crate::context::DbConnection,
    prepared: PreparedVariant,
) -> diesel::QueryResult<()> {
    let PreparedVariant {
        insert_report_benchmark,
        measures,
    } = prepared;

    diesel::insert_into(schema::report_benchmark::table)
        .values(&insert_report_benchmark)
        .execute(conn)?;
    let report_benchmark_id: ReportBenchmarkId =
        diesel::select(last_insert_rowid()).get_result(conn)?;

    for prepared_measure in measures {
        let PreparedMeasure {
            measure_id,
            named,
            mut detections,
        } = prepared_measure;

        let mut insert_named = Vec::with_capacity(named.len());
        for (name, value) in named {
            let insert_metric = InsertMetric::named(
                report_benchmark_id,
                measure_id,
                name.clone(),
                value.into_inner(),
            );
            let Some(prepared_detections) = detections.remove(&name) else {
                insert_named.push(insert_metric);
                continue;
            };
            diesel::insert_into(schema::metric::table)
                .values(&insert_metric)
                .execute(conn)?;
            let metric_id = diesel::select(last_insert_rowid()).get_result(conn)?;
            for prepared_detection in prepared_detections {
                prepared_detection.write(conn, metric_id)?;
            }
        }

        if !insert_named.is_empty() {
            diesel::insert_into(schema::metric::table)
                .values(&insert_named)
                .execute(conn)?;
        }
    }

    Ok(())
}

/// Pre-computed data for a single measure at one variant.
struct PreparedMeasure {
    measure_id: MeasureId,
    /// Every metric the measure reported, in lexicographic order.
    named: NamedMap,
    detections: HashMap<MetricName, Vec<PreparedDetection>>,
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
