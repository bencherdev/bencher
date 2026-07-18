#[cfg(feature = "plus")]
use bencher_json::runner::job::{JobUuid, JsonNewRunJob};
use std::collections::HashSet;

use bencher_json::{
    DateTime, JsonNewReport, JsonReport, JsonReportAlertsCounts, JsonReportCounts,
    JsonReportIterationCounts, ReportUuid,
    project::{
        alert::AlertStatus,
        report::{
            Adapter, Iteration, JsonReportAlerts, JsonReportMeasure, JsonReportResult,
            JsonReportResults, JsonReportSettings, ReportIdempotencyKey,
        },
    },
};
use diesel::OptionalExtension as _;
use diesel::{
    AggregateExpressionMethods as _, ExpressionMethods as _, NullableExpressionMethods as _,
    QueryDsl as _, RunQueryDsl as _, SelectableHelper as _,
};

use dropshot::HttpError;
use results::ReportResults;
use slog::Logger;

#[cfg(feature = "plus")]
use crate::macros::sql::last_insert_rowid;
use crate::model::spec::SpecId;
#[cfg(feature = "plus")]
use crate::model::{
    organization::plan::PlanKind,
    project::{
        series::count_active,
        testbed::{RunJob, RunTestbed},
    },
    runner::{PendingInsertJob, SourceIp},
};
use crate::{
    actor_conn,
    context::{ApiContext, DbConnection},
    error::{issue_error, resource_conflict_err, resource_not_found_err},
    macros::fn_get::{fn_get_id, fn_get_uuid},
    model::{
        project::{
            ProjectId, QueryProject,
            benchmark::QueryBenchmark,
            branch::version::{InsertVersion, QueryVersion},
            measure::QueryMeasure,
            testbed::{QueryTestbed, ResolvedTestbed, TestbedId},
            threshold::{QueryThreshold, alert::QueryAlert, model::QueryModel},
        },
        user::{QueryUser, UserId, actor::ApiActor},
    },
    schema::{self, report as report_table},
    view, write_transaction,
};

/// Encapsulates all context from a run request for report creation.
pub struct NewRunReport {
    pub report: JsonNewReport,
    pub idempotency_key: Option<ReportIdempotencyKey>,
    #[cfg(feature = "plus")]
    pub is_claimed: bool,
    #[cfg(feature = "plus")]
    pub testbed: RunTestbed,
    #[cfg(feature = "plus")]
    pub spec_reset: bool,
    #[cfg(feature = "plus")]
    pub job: Option<NewRunJob>,
}

/// Job-related context for a run.
#[cfg(feature = "plus")]
pub struct NewRunJob {
    pub is_claimed: bool,
    pub run_job: JsonNewRunJob,
    pub source_ip: SourceIp,
}

#[cfg(feature = "plus")]
impl NewRunJob {
    pub fn run_job(&self) -> RunJob<'_> {
        match self.run_job.spec.as_ref() {
            Some(spec) => RunJob::WithSpec(spec),
            None => RunJob::WithoutSpec,
        }
    }
}

use super::{
    branch::{BranchId, QueryBranch, head::HeadId, version::VersionId},
    metric::QueryMetric,
    metric_boundary::QueryMetricBoundary,
    threshold::{InsertThreshold, boundary::QueryBoundary},
};

pub mod report_benchmark;
pub mod results;

crate::macros::typed_id::typed_id!(ReportId);

#[derive(diesel::Queryable, diesel::Identifiable, diesel::Associations, diesel::Selectable)]
#[diesel(table_name = report_table)]
#[diesel(belongs_to(QueryProject, foreign_key = project_id))]
pub struct QueryReport {
    pub id: ReportId,
    pub uuid: ReportUuid,
    pub idempotency_key: Option<ReportIdempotencyKey>,
    pub user_id: Option<UserId>,
    pub project_id: ProjectId,
    pub head_id: HeadId,
    pub version_id: VersionId,
    pub testbed_id: TestbedId,
    pub spec_id: Option<SpecId>,
    pub adapter: Adapter,
    pub start_time: DateTime,
    pub end_time: DateTime,
    pub created: DateTime,
}

impl QueryReport {
    fn_get_id!(report, ReportId, ReportUuid);
    fn_get_uuid!(report, ReportId, ReportUuid);

    #[expect(
        clippy::too_many_lines,
        clippy::cognitive_complexity,
        reason = "report creation has many dimensions and steps"
    )]
    pub async fn create(
        log: &Logger,
        context: &ApiContext,
        query_project: &QueryProject,
        new_run_report: NewRunReport,
        api_actor: &ApiActor,
    ) -> Result<JsonReport, HttpError> {
        #[cfg(feature = "otel")]
        let create_start = context.clock.now();

        #[cfg(feature = "plus")]
        InsertReport::rate_limit(context, query_project.id).await?;

        // Check to see if the project is public or private
        // If private, then validate that there is an active subscription or license
        #[cfg(feature = "plus")]
        let plan_kind = PlanKind::new_for_project(
            context,
            context.biller.as_ref(),
            &context.licensor,
            query_project,
            api_actor,
        )
        .await?;
        let project_id = query_project.id;

        let NewRunReport {
            report: mut json_report,
            idempotency_key,
            #[cfg(feature = "plus")]
            is_claimed,
            #[cfg(feature = "plus")]
                testbed: run_testbed,
            #[cfg(feature = "plus")]
            spec_reset,
            #[cfg(feature = "plus")]
                job: new_run_job,
        } = new_run_report;

        // Idempotency check: if a key is provided, look for an existing report
        if let Some(existing) =
            Self::check_idempotency(actor_conn!(context, api_actor), project_id, idempotency_key)?
        {
            return existing.into_json(log, actor_conn!(context, api_actor), ReportMode::Full);
        }

        #[cfg(all(feature = "plus", not(feature = "otel")))]
        let _ = is_claimed;
        #[cfg(all(feature = "plus", feature = "otel"))]
        let priority = plan_kind.priority(is_claimed);

        #[cfg(all(feature = "otel", feature = "plus"))]
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::Run(priority));

        #[cfg(feature = "plus")]
        let run_job = new_run_job
            .as_ref()
            .map_or(RunJob::None, NewRunJob::run_job);

        // Get or create the branch and testbed
        let (branch_id, head_id) = QueryBranch::get_or_create(
            log,
            context,
            project_id,
            &json_report.branch,
            json_report.start_point.as_ref(),
        )
        .await?;
        let ResolvedTestbed {
            testbed_id,
            spec_id,
        } = QueryTestbed::get_or_create(
            context,
            project_id,
            &json_report.testbed,
            #[cfg(feature = "plus")]
            &run_testbed,
            #[cfg(feature = "plus")]
            spec_reset,
            #[cfg(feature = "plus")]
            &run_job,
        )
        .await?;

        // Insert the thresholds for the report
        InsertThreshold::from_report_json(
            log,
            context,
            project_id,
            branch_id,
            testbed_id,
            json_report.thresholds.take(),
        )
        .await?;

        let json_settings = json_report.settings.take().unwrap_or_default();
        let adapter = json_settings.adapter.unwrap_or_default().normalize();

        // Validate job before inserting report so that report + job creation is atomic:
        // if OCI resolution fails, neither the report nor the job is created.
        #[cfg(feature = "plus")]
        let pending_job = if let (Some(spec_id), Some(new_run_job)) = (spec_id, new_run_job) {
            Some(
                PendingInsertJob::from_run(
                    context,
                    query_project,
                    new_run_job.source_ip,
                    spec_id,
                    &plan_kind,
                    new_run_job.is_claimed,
                    new_run_job.run_job,
                    &json_settings,
                )
                .await?,
            )
        } else {
            None
        };

        // Capture whether this is a job-based run before the transaction moves pending_job.
        #[cfg(feature = "plus")]
        let is_job_run = pending_job.is_some();

        // Capture the current time before acquiring the write lock.
        // This is used for the report created timestamp and the job insert timestamp.
        let now = context.clock.now();

        // Pre-check: if a git hash is provided, try to find an existing version
        // via the read pool *before* acquiring the write lock. This avoids holding
        // the write lock for a read-only query in the common case (same hash re-submitted).
        let existing_version_id = if let Some(hash) = json_report.hash.as_ref() {
            QueryVersion::find_by_hash(
                actor_conn!(context, api_actor),
                project_id,
                head_id,
                hash,
            )
            .ok()
            // This is an optimization for the common case (same hash re-submitted).
            // If this version is deleted between this read and the write transaction,
            // the transaction will fail — the same as any other referenced entity
            // (branch, testbed, etc.) being deleted mid-request.
            .flatten()
        } else {
            None
        };

        // Single transaction wraps version + report + job for true atomicity.
        // If any insert fails, all are rolled back.
        let insert_report_uuid = write_transaction!(context, |conn| {
            // If the version was already found outside the transaction, use it.
            // Otherwise, increment a new version inside the transaction.
            let version_id = if let Some(version_id) = existing_version_id {
                version_id
            } else {
                InsertVersion::increment(conn, project_id, head_id, json_report.hash.clone())?
            };

            // Create a new report and add it to the database
            let insert_report = InsertReport::from_json(
                idempotency_key,
                api_actor.user_id(),
                project_id,
                head_id,
                version_id,
                testbed_id,
                spec_id,
                &json_report,
                adapter,
                now,
            );

            diesel::insert_into(schema::report::table)
                .values(&insert_report)
                .execute(conn)?;

            #[cfg(feature = "plus")]
            if let Some(pending_job) = pending_job {
                let report_id = diesel::select(last_insert_rowid()).get_result::<ReportId>(conn)?;
                pending_job.insert(conn, report_id, now)?;
            }

            diesel::QueryResult::Ok(insert_report.uuid)
        })
        .map_err(resource_conflict_err!(Report, &json_report))?;

        // Read full report via public_conn (outside write lock)
        let query_report = schema::report::table
            .filter(schema::report::uuid.eq(&insert_report_uuid))
            .first::<QueryReport>(actor_conn!(context, api_actor))
            .map_err(|e| {
                issue_error(
                    "Failed to find new report that was just created",
                    &format!("Failed to find new report ({insert_report_uuid}) in project ({project_id}) on Bencher even though it was just created."),
                    e,
                )
            })?;

        #[cfg(feature = "plus")]
        if is_job_run {
            // Job-based run: results will be processed in handle_completed()
            return query_report
                .finish_create(
                    log,
                    context,
                    api_actor,
                    #[cfg(feature = "otel")]
                    create_start,
                )
                .await;
        }

        // Process and record the report results
        let results_array: Vec<&str> = json_report.results.iter().map(AsRef::as_ref).collect();
        query_report
            .process_results(
                log,
                context,
                branch_id,
                &results_array,
                adapter,
                json_settings,
                #[cfg(feature = "plus")]
                plan_kind,
                #[cfg(all(feature = "plus", feature = "otel"))]
                priority,
                #[cfg(feature = "plus")]
                query_project,
            )
            .await?;

        // If the report was processed successfully, then return the report with the results
        query_report
            .finish_create(
                log,
                context,
                api_actor,
                #[cfg(feature = "otel")]
                create_start,
            )
            .await
    }

    /// If an idempotency key is provided, check for an existing report with the same key.
    fn check_idempotency(
        conn: &mut DbConnection,
        project_id: ProjectId,
        idempotency_key: Option<ReportIdempotencyKey>,
    ) -> Result<Option<Self>, HttpError> {
        let Some(idempotency_key) = idempotency_key else {
            return Ok(None);
        };
        schema::report::table
            .filter(schema::report::project_id.eq(project_id))
            .filter(schema::report::idempotency_key.eq(idempotency_key))
            .first::<Self>(conn)
            .optional()
            .map_err(|e| {
                issue_error(
                    "Failed to check idempotency key",
                    "Failed to check report idempotency key",
                    e,
                )
            })
    }

    async fn finish_create(
        self,
        log: &Logger,
        context: &ApiContext,
        api_actor: &ApiActor,
        #[cfg(feature = "otel")] create_start: DateTime,
    ) -> Result<JsonReport, HttpError> {
        #[cfg(feature = "otel")]
        {
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::ReportCreate);
            let duration_secs = create_start.elapsed_secs(context.clock.now());
            bencher_otel::ApiMeter::record(
                bencher_otel::ApiHistogram::ReportCreateDuration,
                duration_secs,
            );
        }

        self.into_json(log, actor_conn!(context, api_actor), ReportMode::Full)
    }

    pub fn into_json(
        self,
        log: &Logger,
        conn: &mut DbConnection,
        mode: ReportMode,
    ) -> Result<JsonReport, HttpError> {
        let Self {
            id,
            uuid,
            idempotency_key: _,
            user_id,
            project_id,
            head_id,
            version_id,
            testbed_id,
            spec_id,
            adapter,
            start_time,
            end_time,
            created,
        } = self;

        let query_project = QueryProject::get(conn, project_id)?;
        let user = if let Some(user_id) = user_id {
            Some(QueryUser::get(conn, user_id)?.into_pub_json())
        } else {
            None
        };
        let branch = QueryBranch::get_json_for_report(conn, &query_project, head_id, version_id)?;
        let testbed = QueryTestbed::get_json_for_report(conn, &query_project, testbed_id, spec_id)?;
        let (results, alerts, counts) = match mode {
            ReportMode::Full => {
                let results = get_report_results(log, conn, &query_project, id)?;
                let alerts =
                    get_report_alerts(conn, &query_project, id, head_id, version_id, spec_id)?;
                let counts = report_counts(&results, &alerts);
                (Some(results), Some(alerts), counts)
            },
            ReportMode::Collapsed => (None, None, get_report_counts(conn, id)?),
        };
        #[cfg(feature = "plus")]
        let job = get_report_job(conn, id)?;

        let project = query_project.into_json(conn)?;
        let adapter = adapter.normalize();
        Ok(JsonReport {
            uuid,
            user,
            project,
            branch,
            testbed,
            start_time,
            end_time,
            adapter,
            results,
            alerts,
            counts,
            #[cfg(feature = "plus")]
            job,
            created,
        })
    }

    /// Process benchmark results and record metrics/alerts for this report.
    ///
    /// Shared between `create()` (local runs) and `handle_completed()` (job-based runs).
    /// Includes plan usage tracking and validation.
    #[expect(clippy::too_many_arguments, reason = "cfg features add extra params")]
    pub async fn process_results(
        &self,
        log: &Logger,
        context: &ApiContext,
        branch_id: BranchId,
        results: &[&str],
        adapter: Adapter,
        settings: JsonReportSettings,
        #[cfg(feature = "plus")] plan_kind: PlanKind,
        #[cfg(all(feature = "plus", feature = "otel"))] priority: bencher_json::Priority,
        #[cfg(feature = "plus")] query_project: &QueryProject,
    ) -> Result<(), HttpError> {
        #[cfg(feature = "plus")]
        let mut usage = 0;
        // Capture the Pro active-series billing context (customer + period) before
        // `check_usage` consumes the plan kind; `None` for any non-Pro plan.
        #[cfg(feature = "plus")]
        let series_billing = plan_kind.metered_series_billing();

        let mut report_results = ReportResults::new(
            self.project_id,
            branch_id,
            self.head_id,
            self.testbed_id,
            self.spec_id,
            self.id,
            #[cfg(feature = "plus")]
            results::SeriesCacheContext {
                organization_id: query_project.organization_id,
                report_created: self.created,
            },
        );
        let processed = report_results
            .process(
                log,
                context,
                results,
                adapter,
                settings,
                #[cfg(feature = "plus")]
                &mut usage,
            )
            .await;

        #[cfg(all(feature = "otel", feature = "plus"))]
        if usage > 0 {
            bencher_otel::ApiMeter::increment_by(
                bencher_otel::ApiCounter::MetricsCreate(priority),
                u64::from(usage),
            );
        }

        #[cfg(feature = "plus")]
        plan_kind
            .check_usage(context.biller.as_ref(), query_project, usage)
            .await?;

        // Pro active-series billing: once the report's metrics and series cache are
        // committed, count the organization's period-to-date active series on the request
        // connection and post it to Stripe in a detached task (only the Stripe post is
        // off the hot path). Skipped if processing failed (the cache rolled back).
        #[cfg(feature = "plus")]
        if processed.is_ok()
            && let Some((customer_id, period_start, period_end)) = series_billing
        {
            post_series_usage(
                log,
                context,
                query_project,
                customer_id,
                period_start,
                period_end,
            )
            .await;
        }

        processed
    }
}

/// Post a Pro organization's period-to-date active-series count to the `active_series`
/// meter after a report. Counts on the request connection (a single indexed range scan);
/// only the Stripe meter post is detached, so it never blocks returning the report.
/// Best-effort: a count or post failure is logged (and reported via
/// `ActiveSeriesBilledFailed`) but never surfaced. The next report re-posts the
/// cumulative count, so a dropped post self-heals within the period. The one gap is a
/// failed post on a period's final report with no later report before the period closes,
/// which under-bills by the delta: an accepted availability-over-accuracy tradeoff now
/// that the reconciliation sweep is gone.
#[cfg(feature = "plus")]
async fn post_series_usage(
    log: &Logger,
    context: &ApiContext,
    query_project: &QueryProject,
    customer_id: bencher_billing::CustomerId,
    period_start: DateTime,
    period_end: DateTime,
) {
    let Some(biller) = context.biller.clone() else {
        return;
    };
    let organization_id = query_project.organization_id;
    // Acquire a read connection and count on the request path (a single indexed range
    // scan). Best-effort: a connection or count failure logs and returns without
    // surfacing, since the report itself is already committed.
    let mut conn = match context.database.get_public_conn().await {
        Ok(conn) => conn,
        Err(e) => {
            slog::warn!(
                log,
                "Failed to acquire a connection to count active series for organization ({organization_id}): {e}"
            );
            return;
        },
    };
    let count = match count_active(&mut conn, organization_id, period_start, period_end) {
        Ok(count) => count,
        Err(e) => {
            slog::warn!(
                log,
                "Failed to count active series for organization ({organization_id}): {e}"
            );
            return;
        },
    };
    drop(conn);

    // Stamp the meter post with the count time so the `active_series` meter's `last`
    // aggregation orders deterministically across concurrent reports.
    let counted_at = context.clock.now();
    let log = log.clone();
    tokio::spawn(async move {
        if let Err(e) = biller
            .record_series_usage(&customer_id, count, counted_at)
            .await
        {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::ActiveSeriesBilledFailed);
            slog::warn!(
                log,
                "Failed to record active-series usage for organization ({organization_id}): {e}"
            );
            #[cfg(feature = "sentry")]
            sentry::capture_error(&e);
        } else {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::ActiveSeriesBilled);
        }
    });
}

/// Whether to materialize the full results and alerts when converting a report to JSON.
#[derive(Debug, Clone, Copy)]
pub enum ReportMode {
    /// Include the full report results and alerts and compute the counts from them.
    Full,
    /// Omit the report results and alerts and compute the counts with aggregate queries.
    Collapsed,
}

type ResultsQuery = (
    Iteration,
    QueryBenchmark,
    QueryMeasure,
    QueryMetricBoundary,
    Option<(QueryThreshold, QueryModel)>,
);

fn get_report_results(
    log: &Logger,
    conn: &mut DbConnection,
    project: &QueryProject,
    report_id: ReportId,
) -> Result<JsonReportResults, HttpError> {
    schema::report_benchmark::table
    .filter(schema::report_benchmark::report_id.eq(report_id))
    .inner_join(schema::benchmark::table)
    .inner_join(view::metric_boundary::table
        .inner_join(schema::measure::table)
        // There may or may not be a boundary for any given metric
        .left_join(schema::threshold::table)
        .left_join(schema::model::table)
    )
    // It is important to order by the iteration first in order to make sure they are grouped together below
    // Then ordering by benchmark and finally measure name makes sure that the benchmarks are in the same order for each iteration
    .order((schema::report_benchmark::iteration, schema::benchmark::name, schema::measure::name))
    .select((
        schema::report_benchmark::iteration,
        QueryBenchmark::as_select(),
        QueryMeasure::as_select(),
        QueryMetricBoundary::as_select(),
        (
            (
                schema::threshold::id,
                schema::threshold::uuid,
                schema::threshold::project_id,
                schema::threshold::measure_id,
                schema::threshold::branch_id,
                schema::threshold::testbed_id,
                schema::threshold::model_id,
                schema::threshold::created,
                schema::threshold::modified,
            ),
            (
                schema::model::id,
                schema::model::uuid,
                schema::model::threshold_id,
                schema::model::test,
                schema::model::min_sample_size,
                schema::model::max_sample_size,
                schema::model::window,
                schema::model::lower_boundary,
                schema::model::upper_boundary,
                schema::model::created,
                schema::model::replaced,
            )
        ).nullable(),
    ))
    .load::<ResultsQuery>(conn)
    .map(|results| into_report_results_json(log, project, results))
    .map_err(resource_not_found_err!(ReportBenchmark, project))
}

fn into_report_results_json(
    log: &Logger,
    project: &QueryProject,
    results: Vec<ResultsQuery>,
) -> JsonReportResults {
    let mut report_results = Vec::new();
    let mut report_iteration = Vec::new();
    let mut prev_iteration = None;
    let mut report_result: Option<JsonReportResult> = None;
    for (iteration, query_benchmark, query_measure, query_metric_boundary, threshold_model) in
        results
    {
        // If onto a new iteration, then add the result to the report iteration list.
        // Then add the report iteration list to the report results list.
        if let Some(prev_iteration) = prev_iteration.take()
            && iteration != prev_iteration
        {
            slog::trace!(log, "Iteration {prev_iteration} => {iteration}");
            if let Some(result) = report_result.take() {
                report_iteration.push(result);
            }
            if !report_iteration.is_empty() {
                report_results.push(std::mem::take(&mut report_iteration));
            }
        }
        prev_iteration = Some(iteration);

        // If there is a current report result, make sure that the benchmark is the same.
        // Otherwise, add it to the report iteration list.
        if let Some(result) = report_result.take() {
            if query_benchmark.uuid == result.benchmark.uuid {
                report_result = Some(result);
            } else {
                slog::trace!(
                    log,
                    "Benchmark {} => {}",
                    result.benchmark.uuid,
                    query_benchmark.uuid,
                );
                report_iteration.push(result);
            }
        }

        let (query_metric, query_boundary) = query_metric_boundary.split();
        let report_measure = JsonReportMeasure {
            measure: query_measure.into_json_for_project(project),
            metric: query_metric.into_json(),
            threshold: threshold_model.map(|(threshold, model)| {
                threshold.into_threshold_model_json_for_project(project, model)
            }),
            boundary: query_boundary.map(QueryBoundary::into_json),
        };

        // If there is a current report result, add the report measure to it.
        // Otherwise, create a new report result and add the report measure to it.
        if let Some(result) = report_result.as_mut() {
            result.measures.push(report_measure);
        } else {
            report_result = Some(JsonReportResult {
                iteration,
                benchmark: query_benchmark.into_json_for_project(project),
                measures: vec![report_measure],
            });
        }
    }

    // Save from the last iteration
    if let Some(result) = report_result.take() {
        report_iteration.push(result);
    }
    if !report_iteration.is_empty() {
        report_results.push(report_iteration);
    }
    slog::trace!(log, "Report results: {report_results:#?}");

    report_results
}

/// Compute the report counts with aggregate queries, without loading the results or alerts.
fn get_report_counts(
    conn: &mut DbConnection,
    report_id: ReportId,
) -> Result<JsonReportCounts, HttpError> {
    // Only count benchmarks that have at least one metric,
    // matching the inner join used to build the full results JSON.
    let results = schema::report_benchmark::table
        .filter(schema::report_benchmark::report_id.eq(report_id))
        .inner_join(schema::metric::table)
        .group_by(schema::report_benchmark::iteration)
        .order(schema::report_benchmark::iteration.asc())
        .select((
            schema::report_benchmark::iteration,
            diesel::dsl::count(schema::report_benchmark::benchmark_id).aggregate_distinct(),
            diesel::dsl::count(schema::metric::measure_id).aggregate_distinct(),
        ))
        .load::<(Iteration, i64, i64)>(conn)
        .map_err(resource_not_found_err!(ReportBenchmark, report_id))?
        .into_iter()
        .map(
            |(_iteration, benchmarks, measures)| JsonReportIterationCounts {
                benchmarks: u32::try_from(benchmarks).unwrap_or(u32::MAX),
                measures: u32::try_from(measures).unwrap_or(u32::MAX),
            },
        )
        .collect();

    let mut alerts = JsonReportAlertsCounts::default();
    schema::alert::table
        .inner_join(
            schema::boundary::table
                .inner_join(schema::metric::table.inner_join(schema::report_benchmark::table)),
        )
        .filter(schema::report_benchmark::report_id.eq(report_id))
        .group_by(schema::alert::status)
        .select((schema::alert::status, diesel::dsl::count_star()))
        .load::<(AlertStatus, i64)>(conn)
        .map_err(resource_not_found_err!(Alert, report_id))?
        .into_iter()
        .for_each(|(status, count)| {
            // The GROUP BY yields at most one row per status,
            // so a direct assignment suffices for the active count.
            let count = u32::try_from(count).unwrap_or(u32::MAX);
            alerts.total = alerts.total.saturating_add(count);
            if matches!(status, AlertStatus::Active) {
                alerts.active = count;
            }
        });

    Ok(JsonReportCounts { results, alerts })
}

/// Compute the report counts from already loaded results and alerts.
fn report_counts(results: &JsonReportResults, alerts: &JsonReportAlerts) -> JsonReportCounts {
    let results = results
        .iter()
        .map(|iteration| {
            let measures = iteration
                .iter()
                .flat_map(|result| {
                    result
                        .measures
                        .iter()
                        .map(|report_measure| report_measure.measure.uuid)
                })
                .collect::<HashSet<_>>();
            JsonReportIterationCounts {
                benchmarks: u32::try_from(iteration.len()).unwrap_or(u32::MAX),
                measures: u32::try_from(measures.len()).unwrap_or(u32::MAX),
            }
        })
        .collect();

    let active = alerts
        .iter()
        .filter(|alert| matches!(alert.status, AlertStatus::Active))
        .count();
    JsonReportCounts {
        results,
        alerts: JsonReportAlertsCounts {
            total: u32::try_from(alerts.len()).unwrap_or(u32::MAX),
            active: u32::try_from(active).unwrap_or(u32::MAX),
        },
    }
}

#[cfg(feature = "plus")]
fn get_report_job(
    conn: &mut DbConnection,
    report_id: ReportId,
) -> Result<Option<JobUuid>, HttpError> {
    schema::job::table
        .filter(schema::job::report_id.eq(report_id))
        .select(schema::job::uuid)
        .first(conn)
        .optional()
        .map_err(|e| {
            issue_error(
                "Failed to query job for report",
                &format!("report id: {report_id}"),
                e,
            )
        })
}

fn get_report_alerts(
    conn: &mut DbConnection,
    project: &QueryProject,
    report_id: ReportId,
    head_id: HeadId,
    version_id: VersionId,
    spec_id: Option<SpecId>,
) -> Result<JsonReportAlerts, HttpError> {
    let alerts = schema::alert::table
        .inner_join(
            schema::boundary::table.inner_join(
                schema::metric::table.inner_join(
                    schema::report_benchmark::table
                        .inner_join(schema::report::table)
                        .inner_join(schema::benchmark::table),
                ),
            ),
        )
        .filter(schema::report::id.eq(report_id))
        .order((schema::report_benchmark::iteration, schema::benchmark::name))
        .select((
            schema::report::uuid,
            schema::report::created,
            schema::report_benchmark::iteration,
            QueryAlert::as_select(),
            QueryBenchmark::as_select(),
            QueryMetric::as_select(),
            QueryBoundary::as_select(),
        ))
        .load::<(
            ReportUuid,
            DateTime,
            Iteration,
            QueryAlert,
            QueryBenchmark,
            QueryMetric,
            QueryBoundary,
        )>(conn)
        .map_err(resource_not_found_err!(Alert, report_id))?;

    let mut report_alerts = Vec::new();
    for (
        report_uuid,
        created,
        iteration,
        query_alert,
        query_benchmark,
        query_metric,
        query_boundary,
    ) in alerts
    {
        let json_alert = query_alert.into_json_for_report(
            conn,
            project,
            report_uuid,
            created,
            head_id,
            version_id,
            spec_id,
            iteration,
            query_benchmark,
            query_metric,
            query_boundary,
        )?;
        report_alerts.push(json_alert);
    }

    Ok(report_alerts)
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = report_table)]
pub struct InsertReport {
    pub uuid: ReportUuid,
    pub idempotency_key: Option<ReportIdempotencyKey>,
    pub user_id: Option<UserId>,
    pub project_id: ProjectId,
    pub head_id: HeadId,
    pub version_id: VersionId,
    pub testbed_id: TestbedId,
    pub spec_id: Option<SpecId>,
    pub adapter: Adapter,
    pub start_time: DateTime,
    pub end_time: DateTime,
    pub created: DateTime,
}

impl InsertReport {
    #[cfg(feature = "plus")]
    crate::macros::rate_limit::fn_rate_limit!(report, Report);

    #[expect(clippy::too_many_arguments, reason = "report has many dimensions")]
    pub fn from_json(
        idempotency_key: Option<ReportIdempotencyKey>,
        user_id: Option<UserId>,
        project_id: ProjectId,
        head_id: HeadId,
        version_id: VersionId,
        testbed_id: TestbedId,
        spec_id: Option<SpecId>,
        report: &JsonNewReport,
        adapter: Adapter,
        now: DateTime,
    ) -> Self {
        Self {
            uuid: ReportUuid::new(),
            idempotency_key,
            user_id,
            project_id,
            head_id,
            version_id,
            testbed_id,
            spec_id,
            adapter,
            start_time: report.start_time,
            end_time: report.end_time,
            created: now,
        }
    }
}

/// Upsert the metric count summary for a report.
///
/// On first call for a `report_id`, inserts the count.
/// On subsequent calls, atomically adds `metric_count` to the existing total,
/// clamped to `i32::MAX` to prevent silent overflow. Without the clamp, `SQLite`
/// would store the sum as i64 but Diesel reads it back via `sqlite3_value_int()`
/// which silently truncates to 32 bits.
pub fn upsert_metric_count(
    conn: &mut DbConnection,
    report_id: ReportId,
    metric_count: i32,
) -> diesel::QueryResult<()> {
    use crate::macros::sql::min;

    diesel::insert_into(schema::metric_count_by_report::table)
        .values((
            schema::metric_count_by_report::report_id.eq(report_id),
            schema::metric_count_by_report::metric_count.eq(metric_count),
        ))
        .on_conflict(schema::metric_count_by_report::report_id)
        .do_update()
        .set(schema::metric_count_by_report::metric_count.eq(min(
            schema::metric_count_by_report::metric_count + metric_count,
            i32::MAX,
        )))
        .execute(conn)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _, SelectableHelper as _};

    use bencher_json::{
        DateTime, JsonReportAlertsCounts, JsonReportIterationCounts,
        project::{alert::AlertStatus, boundary::BoundaryLimit},
    };

    use crate::{
        context::DbConnection,
        schema,
        test_util::{
            BranchIds, create_alert, create_base_entities, create_benchmark, create_boundary,
            create_branch_with_head, create_head_version, create_measure, create_metric,
            create_model, create_report, create_report_benchmark, create_testbed, create_threshold,
            create_version, setup_test_db,
        },
    };

    use super::{QueryReport, ReportId, ReportMode, get_report_counts, report_counts};
    use crate::macros::sql::last_insert_rowid;
    use crate::model::project::{ProjectId, testbed::TestbedId};

    fn test_logger() -> slog::Logger {
        slog::Logger::root(slog::Discard, slog::o!())
    }

    struct ReportFixture {
        project_id: ProjectId,
        branch: BranchIds,
        testbed_id: TestbedId,
        report_id: ReportId,
    }

    fn create_report_fixture(conn: &mut DbConnection) -> ReportFixture {
        let base = create_base_entities(conn);
        let branch = create_branch_with_head(
            conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "main",
            "main",
            "00000000-0000-0000-0000-000000000020",
        );
        let testbed_id = create_testbed(
            conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000030",
            "localhost",
            "localhost",
        );
        let version_id = create_version(
            conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000040",
            0,
            None,
        );
        create_head_version(conn, branch.head_id, version_id);
        let report_id = create_report(
            conn,
            "00000000-0000-0000-0000-000000000050",
            base.project_id,
            branch.head_id,
            version_id,
            testbed_id,
        );
        ReportFixture {
            project_id: base.project_id,
            branch,
            testbed_id,
            report_id,
        }
    }

    #[test]
    #[expect(clippy::too_many_lines, reason = "test data setup")]
    fn get_report_counts_per_iteration() {
        let mut conn = setup_test_db();
        let fixture = create_report_fixture(&mut conn);

        let measure_one = create_measure(
            &mut conn,
            fixture.project_id,
            "00000000-0000-0000-0000-000000000060",
            "Latency",
            "latency",
        );
        let measure_two = create_measure(
            &mut conn,
            fixture.project_id,
            "00000000-0000-0000-0000-000000000061",
            "Throughput",
            "throughput",
        );
        let benchmark_one = create_benchmark(
            &mut conn,
            fixture.project_id,
            "00000000-0000-0000-0000-000000000070",
            "bench_one",
            "bench-one",
        );
        let benchmark_two = create_benchmark(
            &mut conn,
            fixture.project_id,
            "00000000-0000-0000-0000-000000000071",
            "bench_two",
            "bench-two",
        );

        // Iteration 0: two benchmarks, each with both measures
        let report_benchmark = create_report_benchmark(
            &mut conn,
            "00000000-0000-0000-0000-000000000080",
            fixture.report_id,
            0,
            benchmark_one,
        );
        create_metric(
            &mut conn,
            "00000000-0000-0000-0000-000000000090",
            report_benchmark,
            measure_one,
            1.0,
        );
        create_metric(
            &mut conn,
            "00000000-0000-0000-0000-000000000091",
            report_benchmark,
            measure_two,
            2.0,
        );
        let report_benchmark = create_report_benchmark(
            &mut conn,
            "00000000-0000-0000-0000-000000000081",
            fixture.report_id,
            0,
            benchmark_two,
        );
        create_metric(
            &mut conn,
            "00000000-0000-0000-0000-000000000092",
            report_benchmark,
            measure_one,
            3.0,
        );
        create_metric(
            &mut conn,
            "00000000-0000-0000-0000-000000000093",
            report_benchmark,
            measure_two,
            4.0,
        );

        // Iteration 1: two benchmarks, only the first measure
        let report_benchmark = create_report_benchmark(
            &mut conn,
            "00000000-0000-0000-0000-000000000082",
            fixture.report_id,
            1,
            benchmark_one,
        );
        create_metric(
            &mut conn,
            "00000000-0000-0000-0000-000000000094",
            report_benchmark,
            measure_one,
            5.0,
        );
        let report_benchmark = create_report_benchmark(
            &mut conn,
            "00000000-0000-0000-0000-000000000083",
            fixture.report_id,
            1,
            benchmark_two,
        );
        create_metric(
            &mut conn,
            "00000000-0000-0000-0000-000000000095",
            report_benchmark,
            measure_one,
            6.0,
        );

        // A benchmark with no metrics is excluded, matching the full results JSON
        let benchmark_three = create_benchmark(
            &mut conn,
            fixture.project_id,
            "00000000-0000-0000-0000-000000000072",
            "bench_three",
            "bench-three",
        );
        create_report_benchmark(
            &mut conn,
            "00000000-0000-0000-0000-000000000084",
            fixture.report_id,
            0,
            benchmark_three,
        );

        let counts =
            get_report_counts(&mut conn, fixture.report_id).expect("Failed to get report counts");
        assert_eq!(
            counts.results,
            vec![
                JsonReportIterationCounts {
                    benchmarks: 2,
                    measures: 2,
                },
                JsonReportIterationCounts {
                    benchmarks: 2,
                    measures: 1,
                },
            ]
        );
        assert_eq!(counts.alerts, JsonReportAlertsCounts::default());
    }

    #[test]
    fn get_report_counts_empty_report() {
        let mut conn = setup_test_db();
        let fixture = create_report_fixture(&mut conn);

        let counts =
            get_report_counts(&mut conn, fixture.report_id).expect("Failed to get report counts");
        assert!(counts.results.is_empty());
        assert_eq!(counts.alerts, JsonReportAlertsCounts::default());
    }

    #[test]
    #[expect(clippy::too_many_lines, reason = "test data setup")]
    fn get_report_counts_alerts() {
        let mut conn = setup_test_db();
        let fixture = create_report_fixture(&mut conn);

        let measure_id = create_measure(
            &mut conn,
            fixture.project_id,
            "00000000-0000-0000-0000-000000000060",
            "Latency",
            "latency",
        );
        let benchmark_one = create_benchmark(
            &mut conn,
            fixture.project_id,
            "00000000-0000-0000-0000-000000000070",
            "bench_one",
            "bench-one",
        );
        let benchmark_two = create_benchmark(
            &mut conn,
            fixture.project_id,
            "00000000-0000-0000-0000-000000000071",
            "bench_two",
            "bench-two",
        );
        let threshold_id = create_threshold(
            &mut conn,
            fixture.project_id,
            fixture.branch.branch_id,
            fixture.testbed_id,
            measure_id,
            "00000000-0000-0000-0000-0000000000a0",
        );
        let model_id = create_model(
            &mut conn,
            threshold_id,
            "00000000-0000-0000-0000-0000000000b0",
            0,
        );

        let report_benchmark = create_report_benchmark(
            &mut conn,
            "00000000-0000-0000-0000-000000000080",
            fixture.report_id,
            0,
            benchmark_one,
        );
        let metric_id = create_metric(
            &mut conn,
            "00000000-0000-0000-0000-000000000090",
            report_benchmark,
            measure_id,
            1.0,
        );
        let boundary_id = create_boundary(
            &mut conn,
            "00000000-0000-0000-0000-0000000000c0",
            metric_id,
            threshold_id,
            model_id,
        );
        create_alert(
            &mut conn,
            "00000000-0000-0000-0000-0000000000d0",
            boundary_id,
            BoundaryLimit::Upper,
            AlertStatus::Active,
        );

        let report_benchmark = create_report_benchmark(
            &mut conn,
            "00000000-0000-0000-0000-000000000081",
            fixture.report_id,
            0,
            benchmark_two,
        );
        let metric_id = create_metric(
            &mut conn,
            "00000000-0000-0000-0000-000000000091",
            report_benchmark,
            measure_id,
            2.0,
        );
        let boundary_id = create_boundary(
            &mut conn,
            "00000000-0000-0000-0000-0000000000c1",
            metric_id,
            threshold_id,
            model_id,
        );
        create_alert(
            &mut conn,
            "00000000-0000-0000-0000-0000000000d1",
            boundary_id,
            BoundaryLimit::Upper,
            AlertStatus::Dismissed,
        );

        let counts =
            get_report_counts(&mut conn, fixture.report_id).expect("Failed to get report counts");
        assert_eq!(
            counts.alerts,
            JsonReportAlertsCounts {
                total: 2,
                active: 1,
            }
        );
    }

    #[test]
    #[expect(clippy::too_many_lines, reason = "test data setup")]
    fn into_json_full_and_collapsed_agree() {
        let mut conn = setup_test_db();
        let fixture = create_report_fixture(&mut conn);

        let measure_id = create_measure(
            &mut conn,
            fixture.project_id,
            "00000000-0000-0000-0000-000000000060",
            "Latency",
            "latency",
        );
        let benchmark_one = create_benchmark(
            &mut conn,
            fixture.project_id,
            "00000000-0000-0000-0000-000000000070",
            "bench_one",
            "bench-one",
        );
        let benchmark_two = create_benchmark(
            &mut conn,
            fixture.project_id,
            "00000000-0000-0000-0000-000000000071",
            "bench_two",
            "bench-two",
        );
        let threshold_id = create_threshold(
            &mut conn,
            fixture.project_id,
            fixture.branch.branch_id,
            fixture.testbed_id,
            measure_id,
            "00000000-0000-0000-0000-0000000000a0",
        );
        let model_id = create_model(
            &mut conn,
            threshold_id,
            "00000000-0000-0000-0000-0000000000b0",
            0,
        );

        let report_benchmark = create_report_benchmark(
            &mut conn,
            "00000000-0000-0000-0000-000000000080",
            fixture.report_id,
            0,
            benchmark_one,
        );
        let metric_id = create_metric(
            &mut conn,
            "00000000-0000-0000-0000-000000000090",
            report_benchmark,
            measure_id,
            1.0,
        );
        let boundary_id = create_boundary(
            &mut conn,
            "00000000-0000-0000-0000-0000000000c0",
            metric_id,
            threshold_id,
            model_id,
        );
        create_alert(
            &mut conn,
            "00000000-0000-0000-0000-0000000000d0",
            boundary_id,
            BoundaryLimit::Upper,
            AlertStatus::Active,
        );
        let report_benchmark = create_report_benchmark(
            &mut conn,
            "00000000-0000-0000-0000-000000000081",
            fixture.report_id,
            0,
            benchmark_two,
        );
        create_metric(
            &mut conn,
            "00000000-0000-0000-0000-000000000091",
            report_benchmark,
            measure_id,
            2.0,
        );

        let log = test_logger();
        let load_report = |conn: &mut DbConnection| -> QueryReport {
            schema::report::table
                .filter(schema::report::id.eq(fixture.report_id))
                .select(QueryReport::as_select())
                .first(conn)
                .expect("Failed to load report")
        };

        let full = load_report(&mut conn)
            .into_json(&log, &mut conn, ReportMode::Full)
            .expect("Failed to convert full report");
        let results = full.results.as_ref().expect("Full report missing results");
        let alerts = full.alerts.as_ref().expect("Full report missing alerts");
        assert_eq!(full.counts, report_counts(results, alerts));

        let collapsed = load_report(&mut conn)
            .into_json(&log, &mut conn, ReportMode::Collapsed)
            .expect("Failed to convert collapsed report");
        assert!(collapsed.results.is_none());
        assert!(collapsed.alerts.is_none());

        assert_eq!(full.counts, collapsed.counts);
        assert_eq!(
            full.counts.results,
            vec![JsonReportIterationCounts {
                benchmarks: 2,
                measures: 1,
            }]
        );
        assert_eq!(
            full.counts.alerts,
            JsonReportAlertsCounts {
                total: 1,
                active: 1,
            }
        );
    }

    #[test]
    fn last_insert_rowid_returns_report_id() {
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

        let report_uuid = "00000000-0000-0000-0000-000000000050";

        // Insert a report and immediately call last_insert_rowid inside a transaction
        let (rowid, select_id) = conn
            .immediate_transaction(|conn| {
                diesel::insert_into(schema::report::table)
                    .values((
                        schema::report::uuid.eq(report_uuid),
                        schema::report::project_id.eq(base.project_id),
                        schema::report::head_id.eq(branch.head_id),
                        schema::report::version_id.eq(version_id),
                        schema::report::testbed_id.eq(testbed_id),
                        schema::report::adapter.eq(0),
                        schema::report::start_time.eq(DateTime::TEST),
                        schema::report::end_time.eq(DateTime::TEST),
                        schema::report::created.eq(DateTime::TEST),
                    ))
                    .execute(conn)?;

                let rowid = diesel::select(last_insert_rowid()).get_result::<ReportId>(conn)?;
                let select_id: ReportId = schema::report::table
                    .filter(schema::report::uuid.eq(report_uuid))
                    .select(schema::report::id)
                    .first(conn)?;

                diesel::QueryResult::Ok((rowid, select_id))
            })
            .expect("Transaction failed");

        assert_eq!(rowid, select_id);
    }

    #[test]
    fn last_insert_rowid_matches_second_report() {
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

        // Insert first report
        diesel::insert_into(schema::report::table)
            .values((
                schema::report::uuid.eq("00000000-0000-0000-0000-000000000050"),
                schema::report::project_id.eq(base.project_id),
                schema::report::head_id.eq(branch.head_id),
                schema::report::version_id.eq(version_id),
                schema::report::testbed_id.eq(testbed_id),
                schema::report::adapter.eq(0),
                schema::report::start_time.eq(DateTime::TEST),
                schema::report::end_time.eq(DateTime::TEST),
                schema::report::created.eq(DateTime::TEST),
            ))
            .execute(&mut conn)
            .expect("Failed to insert first report");

        // Insert second report and verify last_insert_rowid points to the second one
        let second_uuid = "00000000-0000-0000-0000-000000000051";
        let (rowid, select_id) = conn
            .immediate_transaction(|conn| {
                diesel::insert_into(schema::report::table)
                    .values((
                        schema::report::uuid.eq(second_uuid),
                        schema::report::project_id.eq(base.project_id),
                        schema::report::head_id.eq(branch.head_id),
                        schema::report::version_id.eq(version_id),
                        schema::report::testbed_id.eq(testbed_id),
                        schema::report::adapter.eq(0),
                        schema::report::start_time.eq(DateTime::TEST),
                        schema::report::end_time.eq(DateTime::TEST),
                        schema::report::created.eq(DateTime::TEST),
                    ))
                    .execute(conn)?;

                let rowid = diesel::select(last_insert_rowid()).get_result::<ReportId>(conn)?;
                let select_id: ReportId = schema::report::table
                    .filter(schema::report::uuid.eq(second_uuid))
                    .select(schema::report::id)
                    .first(conn)?;

                diesel::QueryResult::Ok((rowid, select_id))
            })
            .expect("Transaction failed");

        // last_insert_rowid should match the SECOND report, not the first
        assert_eq!(rowid, select_id);
        // And it should NOT be the first report's id
        let first_id: ReportId = schema::report::table
            .filter(schema::report::uuid.eq("00000000-0000-0000-0000-000000000050"))
            .select(schema::report::id)
            .first(&mut conn)
            .expect("Failed to get first report id");
        assert_ne!(rowid, first_id);
    }

    #[test]
    fn upsert_metric_count_clamps_at_i32_max() {
        use super::upsert_metric_count;

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

        // Insert a report to get a valid ReportId
        let report_id: ReportId = conn
            .immediate_transaction(|conn| {
                diesel::insert_into(schema::report::table)
                    .values((
                        schema::report::uuid.eq("00000000-0000-0000-0000-000000000050"),
                        schema::report::project_id.eq(base.project_id),
                        schema::report::head_id.eq(branch.head_id),
                        schema::report::version_id.eq(version_id),
                        schema::report::testbed_id.eq(testbed_id),
                        schema::report::adapter.eq(0),
                        schema::report::start_time.eq(DateTime::TEST),
                        schema::report::end_time.eq(DateTime::TEST),
                        schema::report::created.eq(DateTime::TEST),
                    ))
                    .execute(conn)?;

                diesel::select(last_insert_rowid()).get_result::<ReportId>(conn)
            })
            .expect("Failed to insert report");

        // First upsert: set metric_count to i32::MAX
        upsert_metric_count(&mut conn, report_id, i32::MAX).expect("First upsert failed");

        let count: i32 = schema::metric_count_by_report::table
            .filter(schema::metric_count_by_report::report_id.eq(report_id))
            .select(schema::metric_count_by_report::metric_count)
            .first(&mut conn)
            .expect("Failed to read metric count");
        assert_eq!(count, i32::MAX);

        // Second upsert: adding 1 would overflow without the clamp
        upsert_metric_count(&mut conn, report_id, 1).expect("Second upsert failed");

        let count: i32 = schema::metric_count_by_report::table
            .filter(schema::metric_count_by_report::report_id.eq(report_id))
            .select(schema::metric_count_by_report::metric_count)
            .first(&mut conn)
            .expect("Failed to read metric count after overflow attempt");
        // Should be clamped at i32::MAX, not wrapped/truncated
        assert_eq!(count, i32::MAX);
    }
}
