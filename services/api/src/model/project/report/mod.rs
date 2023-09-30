use std::str::FromStr;

use bencher_json::{
    project::report::{JsonAdapter, JsonReportAlerts, JsonReportResult, JsonReportResults},
    JsonNewReport, JsonReport,
};
use chrono::Utc;
use diesel::{
    ExpressionMethods, NullableExpressionMethods, QueryDsl, RunQueryDsl, SelectableHelper,
};
use slog::Logger;
use uuid::Uuid;

use crate::{
    context::DbConnection,
    model::{
        project::{
            benchmark::QueryBenchmark,
            branch::{BranchId, QueryBranch},
            metric::QueryMetric,
            metric_kind::QueryMetricKind,
            testbed::{QueryTestbed, TestbedId},
            threshold::statistic::QueryStatistic,
            threshold::{alert::QueryAlert, boundary::QueryBoundary, QueryThreshold},
            version::VersionId,
            ProjectId, QueryProject,
        },
        user::{QueryUser, UserId},
    },
    schema,
    schema::report as report_table,
    util::{query::fn_get_id, to_date_time},
    ApiError,
};

mod adapter;
pub mod results;

use adapter::Adapter;

crate::util::typed_id::typed_id!(ReportId);

#[derive(diesel::Queryable, diesel::Identifiable, diesel::Associations, diesel::Selectable)]
#[diesel(table_name = report_table)]
#[diesel(belongs_to(QueryProject, foreign_key = project_id))]
pub struct QueryReport {
    pub id: ReportId,
    pub uuid: String,
    pub user_id: UserId,
    pub project_id: ProjectId,
    pub branch_id: BranchId,
    pub version_id: VersionId,
    pub testbed_id: TestbedId,
    pub adapter: Adapter,
    pub start_time: i64,
    pub end_time: i64,
    pub created: i64,
}

impl QueryReport {
    fn_get_id!(report, ReportId);

    pub fn get_uuid(conn: &mut DbConnection, id: ReportId) -> Result<Uuid, ApiError> {
        let uuid: String = schema::report::table
            .filter(schema::report::id.eq(id))
            .select(schema::report::uuid)
            .first(conn)
            .map_err(ApiError::from)?;
        Uuid::from_str(&uuid).map_err(ApiError::from)
    }

    pub fn into_json(self, log: &Logger, conn: &mut DbConnection) -> Result<JsonReport, ApiError> {
        let Self {
            id,
            uuid,
            user_id,
            project_id,
            branch_id,
            version_id,
            testbed_id,
            adapter,
            start_time,
            end_time,
            created,
        } = self;

        let project = QueryProject::get(conn, project_id)?.into_json(conn)?;
        let results = get_report_results(log, conn, project.uuid, id)?;
        let alerts = get_report_alerts(conn, project.uuid, id)?;

        Ok(JsonReport {
            uuid: Uuid::from_str(&uuid).map_err(ApiError::from)?,
            user: QueryUser::get(conn, user_id)?.into_json()?,
            project,
            branch: QueryBranch::get_branch_version_json(conn, branch_id, version_id)?,
            testbed: QueryTestbed::get(conn, testbed_id)?.into_json(conn)?,
            adapter: adapter.into(),
            start_time: to_date_time(start_time)?,
            end_time: to_date_time(end_time)?,
            results,
            alerts,
            created: to_date_time(created).map_err(ApiError::from)?,
        })
    }
}

type ResultsQuery = (
    i32,
    QueryMetricKind,
    Option<(QueryThreshold, QueryStatistic)>,
    QueryBenchmark,
    QueryMetric,
    Option<QueryBoundary>,
);

fn get_report_results(
    log: &Logger,
    conn: &mut DbConnection,
    project: Uuid,
    report_id: ReportId,
) -> Result<JsonReportResults, ApiError> {
    let results = schema::perf::table
    .filter(schema::perf::report_id.eq(report_id))
    .inner_join(schema::benchmark::table)
    .inner_join(schema::metric::table
        .inner_join(schema::metric_kind::table)
        .left_join(schema::boundary::table
            .inner_join(schema::threshold::table)
            .inner_join(schema::statistic::table)
        )
    )
    // It is important to order by the iteration first in order to make sure they are grouped together below
    // Then ordering by metric kind and finally benchmark name makes sure that the benchmarks are in the same order for each iteration
    .order((schema::perf::iteration, schema::metric_kind::name, schema::benchmark::name))
    .select((
        schema::perf::iteration,
        QueryMetricKind::as_select(),
        (
            (
                schema::threshold::id,
                schema::threshold::uuid,
                schema::threshold::project_id,
                schema::threshold::metric_kind_id,
                schema::threshold::branch_id,
                schema::threshold::testbed_id,
                schema::threshold::statistic_id,
                schema::threshold::created,
                schema::threshold::modified,
            ),
            (
                schema::statistic::id,
                schema::statistic::uuid,
                schema::statistic::threshold_id,
                schema::statistic::test,
                schema::statistic::min_sample_size,
                schema::statistic::max_sample_size,
                schema::statistic::window,
                schema::statistic::lower_boundary,
                schema::statistic::upper_boundary,
                schema::statistic::created,
            )
        ).nullable(),
        QueryBenchmark::as_select(),
        QueryMetric::as_select(),
        (
            schema::boundary::id,
            schema::boundary::uuid,
            schema::boundary::threshold_id,
            schema::boundary::statistic_id,
            schema::boundary::metric_id,
            schema::boundary::lower_limit,
            schema::boundary::upper_limit,
        ).nullable(),
    ))
    .load::<ResultsQuery>(conn)
    .map_err(ApiError::from)?;

    into_report_results_json(log, project, results)
}

fn into_report_results_json(
    log: &Logger,
    project: Uuid,
    results: Vec<ResultsQuery>,
) -> Result<JsonReportResults, ApiError> {
    let mut report_results = Vec::new();
    let mut report_iteration = Vec::new();
    let mut prev_iteration = None;
    let mut report_result: Option<JsonReportResult> = None;
    for (
        iteration,
        query_metric_kind,
        threshold_statistic,
        query_benchmark,
        query_metric,
        query_boundary,
    ) in results
    {
        // If onto a new iteration, then add the result to the report iteration list.
        // Then add the report iteration list to the report results list.
        if let Some(prev_iteration) = prev_iteration.take() {
            if iteration != prev_iteration {
                slog::trace!(log, "Iteration {prev_iteration} => {iteration}");
                if let Some(result) = report_result.take() {
                    report_iteration.push(result);
                }
                report_results.push(std::mem::take(&mut report_iteration));
            }
        }
        prev_iteration = Some(iteration);

        // If there is a current report result, make sure that the metric kind is the same.
        // Otherwise, add it to the report iteration list.
        if let Some(result) = report_result.take() {
            if query_metric_kind.uuid == result.metric_kind.uuid.to_string() {
                report_result = Some(result);
            } else {
                slog::trace!(
                    log,
                    "Metric Kind {} => {}",
                    result.metric_kind.uuid,
                    query_metric_kind.uuid,
                );
                report_iteration.push(result);
            }
        }

        // Create a benchmark metric out of the benchmark, metric, and boundary
        let benchmark_metric = query_benchmark.into_benchmark_metric_json_for_project(
            project,
            query_metric,
            query_boundary,
        )?;

        // If there is a current report result, add the benchmark metric to it.
        // Otherwise, create a new report result and add the benchmark to it.
        if let Some(result) = report_result.as_mut() {
            result.benchmarks.push(benchmark_metric);
        } else {
            let metric_kind = query_metric_kind.into_json_for_project(project)?;
            let threshold = if let Some((threshold, statistic)) = threshold_statistic {
                Some(threshold.into_threshold_statistic_json_for_project(project, statistic)?)
            } else {
                None
            };
            report_result = Some(JsonReportResult {
                metric_kind,
                threshold,
                benchmarks: vec![benchmark_metric],
            });
        }
    }

    // Save from the last iteration
    if let Some(result) = report_result.take() {
        report_iteration.push(result);
    }
    report_results.push(report_iteration);
    slog::trace!(log, "Report results: {report_results:#?}");

    Ok(report_results)
}

fn get_report_alerts(
    conn: &mut DbConnection,
    project: Uuid,
    report_id: ReportId,
) -> Result<JsonReportAlerts, ApiError> {
    let alerts = schema::alert::table
        .inner_join(
            schema::boundary::table.inner_join(
                schema::metric::table
                    .inner_join(schema::metric_kind::table)
                    .inner_join(
                        schema::perf::table
                            .inner_join(schema::report::table)
                            .inner_join(schema::benchmark::table),
                    ),
            ),
        )
        .filter(schema::report::id.eq(report_id))
        .order((
            schema::perf::iteration,
            schema::metric_kind::name,
            schema::benchmark::name,
        ))
        .select((
            schema::report::uuid,
            schema::perf::iteration,
            QueryAlert::as_select(),
            QueryBenchmark::as_select(),
            QueryMetric::as_select(),
            QueryBoundary::as_select(),
        ))
        .load::<(
            String,
            i32,
            QueryAlert,
            QueryBenchmark,
            QueryMetric,
            QueryBoundary,
        )>(conn)
        .map_err(ApiError::from)?;

    let mut report_alerts = Vec::new();
    for (report, iteration, query_alert, query_benchmark, query_metric, query_boundary) in alerts {
        let json_alert = query_alert.into_json_for_report(
            conn,
            project,
            report,
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
    pub uuid: String,
    pub user_id: UserId,
    pub project_id: ProjectId,
    pub branch_id: BranchId,
    pub version_id: VersionId,
    pub testbed_id: TestbedId,
    pub adapter: Adapter,
    pub start_time: i64,
    pub end_time: i64,
    pub created: i64,
}

impl InsertReport {
    pub fn from_json(
        user_id: UserId,
        project_id: ProjectId,
        branch_id: BranchId,
        version_id: VersionId,
        testbed_id: TestbedId,
        report: &JsonNewReport,
        adapter: JsonAdapter,
    ) -> Self {
        Self {
            uuid: Uuid::new_v4().to_string(),
            user_id,
            project_id,
            branch_id,
            version_id,
            testbed_id,
            adapter: adapter.into(),
            start_time: report.start_time.timestamp(),
            end_time: report.end_time.timestamp(),
            created: Utc::now().timestamp(),
        }
    }
}
