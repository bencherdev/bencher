use std::collections::{HashMap, HashSet};

use bencher_adapter::{
    AdapterError, AdapterResults, AdapterResultsArray, Settings as AdapterSettings,
    results::{
        adapter_metrics::{AdapterMetrics, NamedMap},
        adapter_results::{MeasureUnits, Skipped},
    },
};
use bencher_json::{
    BenchmarkName, BenchmarkNameId, BmfVersion, DateTime, MeasureNameId, MetricName, ParameterSet,
    ResourceName, Slug,
    project::report::{
        Adapter, Iteration, JsonAverage, JsonReportSettings, MeasureFilter, ParseOutcome,
        ReportWarningResource,
    },
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;
use slog::Logger;

use crate::macros::sql::last_insert_rowid;
use crate::model::spec::SpecId;
#[cfg(feature = "plus")]
use crate::model::{
    organization::OrganizationId,
    project::series::{SeriesKey, upsert_series_last_seen},
};
use crate::{
    auth_conn,
    context::{ApiContext, DbConnection},
    error::{bad_request_error, issue_error, past_the_ceiling},
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

use super::{QueryReport, ReportId, warning::ReportWarnings};

/// `ReportResults` is used to process the report results.
pub struct ReportResults {
    pub project_id: ProjectId,
    pub branch_id: BranchId,
    pub head_id: HeadId,
    pub testbed_id: TestbedId,
    pub spec_id: Option<SpecId>,
    pub report_id: ReportId,
    /// When the API took the report, which its alerts keep and each series takes as `last_seen`,
    /// so a far-future `end_time` cannot dodge active-series billing.
    pub report_created: DateTime,
    /// Denormalized into each `series_last_seen` row so a billing read is a single index scan.
    #[cfg(feature = "plus")]
    pub organization_id: OrganizationId,
    /// `None` is a new item the creation ceiling refused, which the report skips.
    pub benchmark_cache: HashMap<BenchmarkNameId, Option<BenchmarkId>>,
    pub variant_cache: HashMap<(BenchmarkId, ParameterSet), Option<VariantId>>,
    pub measure_cache: HashMap<MeasureNameId, Option<MeasureId>>,
    pub threshold_cache: HashMap<MeasureId, Vec<Threshold>>,
    /// How the results parsed, once [`Self::process`] has parsed them.
    pub parse: Option<ParseRecord>,
}

impl ReportResults {
    pub fn new(
        report: &QueryReport,
        branch_id: BranchId,
        #[cfg(feature = "plus")] organization_id: OrganizationId,
    ) -> Self {
        Self {
            project_id: report.project_id,
            branch_id,
            head_id: report.head_id,
            testbed_id: report.testbed_id,
            spec_id: report.spec_id,
            report_id: report.id,
            report_created: report.created,
            #[cfg(feature = "plus")]
            organization_id,
            benchmark_cache: HashMap::new(),
            variant_cache: HashMap::new(),
            measure_cache: HashMap::new(),
            threshold_cache: HashMap::new(),
            parse: None,
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
        report_version: ReportBmfVersion,
        warnings: &mut ReportWarnings,
        #[cfg(feature = "plus")] usage: &mut u32,
    ) -> Result<(), HttpError> {
        let bmf_version = report_version.version;
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

        let adapter_settings = adapter_settings_at(settings.average, bmf_version, warnings);
        let mut results_array = AdapterResultsArray::new(results_array, adapter, adapter_settings)
            .inspect_err(|_| self.parse = Some(ParseRecord::FAILED))
            .map_err(|e| match e {
                // The adapter is right and the version is wrong, so no adapter hint.
                AdapterError::BmfVersion { parsed, .. } => report_version.refusal(parsed),
                AdapterError::Valid(_) | AdapterError::BenchmarkUnits(_) | AdapterError::Convert(_) => {
                    bad_request_error(format!(
                        "Failed to convert results with adapter ({adapter} | {settings:?}): {e}\n\nAre you sure {adapter} is the right adapter?\nRead more about adapters here: https://bencher.dev/docs/explanation/adapters/"
                    ))
                },
            })?;
        self.parse = Some(ParseRecord::parsed(&results_array, warnings));

        if let Some(measures) = &settings.measures {
            self.keep_measures(context, measures, &mut results_array)
                .await?;
        }

        // The adapter counts; the log line and the counter are here because this is
        // where the providers are in scope.
        let skipped = results_array.skipped();
        let report_id = self.report_id;
        if !skipped.is_empty() {
            slog::warn!(
                log,
                "Skipped what failed to parse or passed the per measure cap for report ({report_id}): {skipped:?}"
            );
        }
        let Skipped {
            benchmarks,
            variants,
            measures,
            metrics,
        } = skipped;
        #[cfg(feature = "otel")]
        if metrics > 0 {
            bencher_otel::ApiMeter::increment_by(
                bencher_otel::ApiCounter::MetricNamesDropped,
                u64::try_from(metrics).unwrap_or(u64::MAX),
            );
        }
        warnings.skip_with_count(ReportWarningResource::Benchmark, benchmarks);
        warnings.skip_with_count(ReportWarningResource::Variant, variants);
        warnings.skip_with_count(ReportWarningResource::Measure, measures);
        warnings.skip_with_count(ReportWarningResource::Metric, metrics);

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
                warnings,
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
        warnings: &mut ReportWarnings,
        #[cfg(feature = "plus")] usage: &mut u32,
    ) -> Result<(), HttpError> {
        let units = results.units;
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
            let Some(benchmark_id) = self.benchmark_id(log, context, benchmark).await? else {
                warnings.skip(ReportWarningResource::Benchmark);
                continue;
            };
            // A benchmark reports one result per variant, and
            // each is its own `report_benchmark` row with its own series history.
            for (parameters, metrics) in entries {
                if skip_empty_variants && metrics.inner.is_empty() {
                    continue;
                }
                let Some(prepared) = self
                    .prepare_variant(
                        log,
                        context,
                        iteration,
                        benchmark_id,
                        ignore_benchmark,
                        parameters,
                        metrics,
                        &units,
                        warnings,
                    )
                    .await?
                else {
                    continue;
                };
                // A v1 variant whose every measure the ceiling refused measured nothing too.
                if skip_empty_variants && prepared.measures.is_empty() {
                    continue;
                }
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
                write_variant(conn, prepared, self.report_created)?;
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
                    self.organization_id,
                    self.project_id,
                    series,
                    self.report_created,
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

    /// Phase 1: Prepare all data for a single variant (reads + compute only),
    /// or `None` if the creation ceiling refused the variant.
    #[expect(
        clippy::too_many_arguments,
        reason = "a variant is a benchmark, its parameters, and its metrics with their units"
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
        units: &MeasureUnits,
        warnings: &mut ReportWarnings,
    ) -> Result<Option<PreparedVariant>, HttpError> {
        // Resolved here in Phase 1, alongside the benchmark and the measures, so the
        // Phase 2 write transaction stays read free and never nests a transaction.
        let Some(variant_id) = self
            .variant_id(log, context, benchmark_id, parameters.clone())
            .await?
        else {
            warnings.skip(ReportWarningResource::Variant);
            return Ok(None);
        };

        let insert_report_benchmark =
            InsertReportBenchmark::from_json(self.report_id, iteration, benchmark_id, variant_id);

        let mut prepared_measures = Vec::with_capacity(metrics.inner.len());
        for (measure_key, metric) in metrics.inner {
            let units = units.get(&measure_key);
            let Some(measure_id) = self.measure_id(log, context, measure_key, units).await? else {
                warnings.skip(ReportWarningResource::Measure);
                continue;
            };
            let named = metric.inner;

            let mut detections: HashMap<MetricName, Vec<PreparedDetection>> = HashMap::new();
            for threshold in self.threshold_cache.get(&measure_id).into_iter().flatten() {
                if !threshold.checks(&parameters) {
                    continue;
                }
                let Some(value) = named.get(&threshold.metric).copied() else {
                    warnings.skip(ReportWarningResource::Threshold);
                    continue;
                };
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

        Ok(Some(PreparedVariant {
            insert_report_benchmark,
            measures: prepared_measures,
        }))
    }

    /// Drop every measure the report does not keep, before anything is stored.
    async fn keep_measures(
        &self,
        context: &ApiContext,
        measures: &MeasureFilter,
        results_array: &mut AdapterResultsArray,
    ) -> Result<(), HttpError> {
        if measures.measures().is_empty() {
            return Ok(());
        }
        let kept = kept_measures(
            auth_conn!(context),
            self.project_id,
            measures,
            results_array,
        )
        .map_err(|e| {
            issue_error(
                "Failed to resolve the kept measures",
                &format!(
                    "Failed to resolve the measures that report ({}) keeps:",
                    self.report_id
                ),
                e,
            )
        })?;
        results_array.retain_measures(|measure| kept.contains(measure));
        Ok(())
    }

    async fn benchmark_id(
        &mut self,
        log: &Logger,
        context: &ApiContext,
        benchmark: BenchmarkNameId,
    ) -> Result<Option<BenchmarkId>, HttpError> {
        Ok(if let Some(id) = self.benchmark_cache.get(&benchmark) {
            *id
        } else {
            let benchmark_id = past_the_ceiling(
                log,
                QueryBenchmark::get_or_create(context, self.project_id, &benchmark).await,
            )?;
            self.benchmark_cache.insert(benchmark, benchmark_id);
            benchmark_id
        })
    }

    /// The variant's row, keyed on both the benchmark and its parameters:
    /// one benchmark has many variants.
    async fn variant_id(
        &mut self,
        log: &Logger,
        context: &ApiContext,
        benchmark_id: BenchmarkId,
        parameters: ParameterSet,
    ) -> Result<Option<VariantId>, HttpError> {
        let key = (benchmark_id, parameters);
        Ok(if let Some(id) = self.variant_cache.get(&key) {
            *id
        } else {
            let variant_id = past_the_ceiling(
                log,
                QueryVariant::get_or_create(context, self.project_id, benchmark_id, &key.1).await,
            )?;
            self.variant_cache.insert(key, variant_id);
            variant_id
        })
    }

    async fn measure_id(
        &mut self,
        log: &Logger,
        context: &ApiContext,
        measure: MeasureNameId,
        units: Option<&ResourceName>,
    ) -> Result<Option<MeasureId>, HttpError> {
        Ok(if let Some(id) = self.measure_cache.get(&measure) {
            *id
        } else {
            let measure_id = past_the_ceiling(
                log,
                QueryMeasure::get_or_create_with_units(context, self.project_id, &measure, units)
                    .await,
            )?;
            self.measure_cache.insert(measure, measure_id);
            measure_id
        })
    }
}

/// The settings a report reads its results with: no average at version 1, where every average
/// a harness reports is its own metric, so a requested one is ignored with a warning.
fn adapter_settings_at(
    average: Option<JsonAverage>,
    bmf_version: BmfVersion,
    warnings: &mut ReportWarnings,
) -> AdapterSettings {
    if bmf_version == BmfVersion::V1 && average.is_some() {
        warnings.ignore_with_count(ReportWarningResource::Average, 1);
        return AdapterSettings::new(None, bmf_version);
    }
    AdapterSettings::new(average, bmf_version)
}

/// The measures of the results that a listed measure names, matched by what each resolves to
/// in the project, so a name and a slug of one measure match.
fn kept_measures(
    conn: &mut DbConnection,
    project_id: ProjectId,
    measures: &MeasureFilter,
    results_array: &AdapterResultsArray,
) -> diesel::QueryResult<HashSet<MeasureNameId>> {
    let mut listed = HashSet::with_capacity(measures.measures().len());
    for measure in measures.measures() {
        if let Some(identity) = QueryMeasure::identity(conn, project_id, measure)? {
            listed.insert(identity);
        }
    }
    let mut kept = HashSet::new();
    for measure in results_array.measures() {
        if let Some(identity) = QueryMeasure::identity(conn, project_id, measure)?
            && listed.contains(&identity)
        {
            kept.insert(measure.clone());
        }
    }
    Ok(kept)
}

/// How a report's results parsed, as the report records it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseRecord {
    pub resolved_adapter: Option<Adapter>,
    pub outcome: ParseOutcome,
}

impl ParseRecord {
    const FAILED: Self = Self {
        resolved_adapter: None,
        outcome: ParseOutcome::Failed,
    };

    /// The record of results that parsed, which keeps the first iteration's leaf
    /// adapter and warns of every iteration that resolved to another.
    fn parsed(results_array: &AdapterResultsArray, warnings: &mut ReportWarnings) -> Self {
        warnings.ignore_with_count(
            ReportWarningResource::Adapter,
            results_array.mixed_adapters(),
        );
        Self {
            resolved_adapter: results_array.resolved_adapter(),
            outcome: if results_array.has_metrics() {
                ParseOutcome::Parsed
            } else {
                ParseOutcome::Empty
            },
        }
    }

    pub fn update(self, conn: &mut DbConnection, report_id: ReportId) -> diesel::QueryResult<()> {
        let Self {
            resolved_adapter,
            outcome,
        } = self;
        diesel::update(schema::report::table.filter(schema::report::id.eq(report_id)))
            .set((
                schema::report::resolved_adapter.eq(resolved_adapter),
                schema::report::parse_outcome.eq(outcome),
            ))
            .execute(conn)?;
        Ok(())
    }
}

/// The BMF version a report reads its results at, and where that version came from.
#[derive(Debug, Clone, Copy)]
pub struct ReportBmfVersion {
    pub version: BmfVersion,
    pub source: BmfVersionSource,
}

impl ReportBmfVersion {
    /// The version a report declared, or else its project's.
    pub fn new(declared: Option<BmfVersion>, project: BmfVersion) -> Self {
        if let Some(version) = declared {
            Self {
                version,
                source: BmfVersionSource::Declared,
            }
        } else {
            Self {
                version: project,
                source: BmfVersionSource::Project,
            }
        }
    }

    /// The version a remote job's results are read at: its report's, then its job config's, then
    /// version 0.
    pub fn job(report: Option<BmfVersion>, config: Option<BmfVersion>) -> Self {
        Self {
            version: report.or(config).unwrap_or_default(),
            source: BmfVersionSource::Job,
        }
    }

    /// The 400 for results that parsed at the other version, naming only the fixes this server
    /// takes.
    fn refusal(self, parsed: BmfVersion) -> HttpError {
        let Self { version, source } = self;
        bad_request_error(match source {
            BmfVersionSource::Declared => format!(
                "Results parsed as BMF version {parsed}, but the report declared `bmf_version` {version}. Send BMF version {version} results, or declare `bmf_version` {parsed}."
            ),
            BmfVersionSource::Project => format!(
                "Results parsed as BMF version {parsed}, but the report declared no `bmf_version`, so it was read at its project's BMF version, {version}. Send BMF version {version} results, or declare `bmf_version` {parsed} through the API. A server admin can also move the project to BMF version {parsed} with `bencher project update --bmf-version {parsed}`."
            ),
            BmfVersionSource::Job => format!(
                "Results parsed as BMF version {parsed}, but the remote job was accepted at BMF version {version}. Send BMF version {version} results, or run the job again at BMF version {parsed}: declare `bmf_version` {parsed} through the API, or, for a run that declares none, have a server admin move the project to BMF version {parsed} with `bencher project update --bmf-version {parsed}`."
            ),
        })
    }
}

/// Where a report's BMF version came from.
#[derive(Debug, Clone, Copy)]
pub enum BmfVersionSource {
    /// The report declared it.
    Declared,
    /// The report declared none, so it is its project's.
    Project,
    /// A remote job's run pinned it when the run was accepted.
    Job,
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
    conn: &mut DbConnection,
    prepared: PreparedVariant,
    report_created: DateTime,
) -> diesel::QueryResult<()> {
    let PreparedVariant {
        insert_report_benchmark,
        measures,
    } = prepared;
    let report_id = insert_report_benchmark.report_id;

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
                prepared_detection.write(conn, report_id, report_created, metric_id)?;
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
    use bencher_adapter::{AdapterResults, AdapterResultsArray};
    use bencher_json::{
        BenchmarkNameId, BmfVersion,
        project::report::{
            Adapter, JsonAverage, JsonReportWarning, ParseOutcome, ReportWarningAction,
            ReportWarningResource,
        },
    };
    use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};

    use super::{ParseRecord, adapter_settings_at, strip_ignore_suffix};
    use crate::{
        model::project::report::warning::{ReportWarnings, get_report_warnings},
        schema,
        test_util::{
            create_base_entities, create_branch_with_head, create_head_version, create_report,
            create_testbed, create_version, setup_test_db,
        },
    };

    /// A report whose iterations resolve to different leaves records the first,
    /// and warns once for each iteration whose leaf it set aside.
    #[test]
    fn parse_record_keeps_the_first_leaf_and_warns_of_the_rest() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let branch = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "main",
            "main",
            "00000000-0000-0000-0000-000000000020",
        );
        let testbed_id = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000030",
            "localhost",
            "localhost",
        );
        let version_id = create_version(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000040",
            0,
            None,
        );
        create_head_version(&mut conn, branch.head_id, version_id);
        let report_id = create_report(
            &mut conn,
            "00000000-0000-0000-0000-000000000050",
            base.project_id,
            branch.head_id,
            version_id,
            testbed_id,
        );

        let results_array = AdapterResultsArray::from_iterations(
            [
                Adapter::JsonV1,
                Adapter::RustBench,
                Adapter::JsonV1,
                Adapter::RustCriterion,
            ]
            .map(|leaf| AdapterResults {
                resolved_adapter: Some(leaf),
                ..AdapterResults::default()
            })
            .to_vec(),
        );
        let mut warnings = ReportWarnings::default();
        let parse = ParseRecord::parsed(&results_array, &mut warnings);
        assert_eq!(
            parse,
            ParseRecord {
                resolved_adapter: Some(Adapter::JsonV1),
                outcome: ParseOutcome::Empty,
            }
        );

        parse
            .update(&mut conn, report_id)
            .expect("Failed to record the parse");
        warnings
            .insert(&mut conn, report_id)
            .expect("Failed to write the warnings");
        let recorded: (Option<Adapter>, Option<ParseOutcome>) = schema::report::table
            .filter(schema::report::id.eq(report_id))
            .select((
                schema::report::resolved_adapter,
                schema::report::parse_outcome,
            ))
            .first(&mut conn)
            .expect("Failed to read the report");
        assert_eq!(recorded, (Some(Adapter::JsonV1), Some(ParseOutcome::Empty)));
        assert_eq!(
            get_report_warnings(&mut conn, report_id).expect("Failed to read the warnings"),
            Some(vec![JsonReportWarning {
                resource: ReportWarningResource::Adapter,
                action: ReportWarningAction::Ignore,
                count: 2,
            }])
        );
    }

    /// Kills: an average at version 1 that still reaches the adapter or is ignored without a
    /// warning, a warning at version 1 with no average, and an average at version 0 that is
    /// dropped, changed, or warned of.
    #[test]
    fn adapter_settings_at_withholds_the_average_at_version_1() {
        let mut warnings = ReportWarnings::default();
        let settings =
            adapter_settings_at(Some(JsonAverage::Median), BmfVersion::V0, &mut warnings);
        assert!(
            matches!(settings.average, Some(JsonAverage::Median)),
            "{settings:?}"
        );
        assert_eq!(settings.bmf_version, BmfVersion::V0);
        assert_eq!(warnings, ReportWarnings::default());

        let settings = adapter_settings_at(None, BmfVersion::V1, &mut warnings);
        assert!(settings.average.is_none(), "{settings:?}");
        assert_eq!(settings.bmf_version, BmfVersion::V1);
        assert_eq!(warnings, ReportWarnings::default());

        let settings =
            adapter_settings_at(Some(JsonAverage::Median), BmfVersion::V1, &mut warnings);
        assert!(settings.average.is_none(), "{settings:?}");
        assert_eq!(settings.bmf_version, BmfVersion::V1);
        let mut expected = ReportWarnings::default();
        expected.ignore_with_count(ReportWarningResource::Average, 1);
        assert_eq!(warnings, expected);
    }

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
