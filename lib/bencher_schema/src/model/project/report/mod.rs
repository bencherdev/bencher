use bencher_json::{
    DateTime, JsonNewReport, JsonReport, ReportUuid,
    project::report::{
        Adapter, Iteration, JsonReportAlerts, JsonReportMeasure, JsonReportResult,
        JsonReportResults,
    },
};
use diesel::{
    ExpressionMethods as _, NullableExpressionMethods as _, QueryDsl as _, RunQueryDsl as _,
    SelectableHelper as _,
};
use dropshot::HttpError;
use results::ReportResults;
use slog::Logger;

#[cfg(feature = "plus")]
use crate::model::organization::plan::PlanKind;
#[cfg(feature = "plus")]
use crate::model::runner::job::QueryJob;
use crate::{
    context::{ApiContext, DbConnection},
    error::{issue_error, resource_conflict_err, resource_not_found_err},
    macros::fn_get::{fn_get_id, fn_get_uuid},
    model::{
        project::{
            ProjectId, QueryProject,
            benchmark::QueryBenchmark,
            branch::version::QueryVersion,
            measure::QueryMeasure,
            testbed::{QueryTestbed, TestbedId},
            threshold::{QueryThreshold, alert::QueryAlert, model::QueryModel},
        },
        user::{QueryUser, UserId, public::PublicUser},
    },
    public_conn,
    schema::{self, report as report_table},
    view, write_conn,
};

use super::{
    branch::{QueryBranch, head::HeadId, version::VersionId},
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
    pub user_id: Option<UserId>,
    pub project_id: ProjectId,
    pub head_id: HeadId,
    pub version_id: VersionId,
    pub testbed_id: TestbedId,
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
        reason = "spec_id binding extraction pushes over limit"
    )]
    pub async fn create(
        log: &Logger,
        context: &ApiContext,
        query_project: &QueryProject,
        mut json_report: JsonNewReport,
        public_user: &PublicUser,
    ) -> Result<JsonReport, HttpError> {
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
            public_user,
        )
        .await?;
        let project_id = query_project.id;

        // Get or create the branch and testbed
        let (branch_id, head_id) = QueryBranch::get_or_create(
            log,
            context,
            project_id,
            &json_report.branch,
            json_report.start_point.as_ref(),
        )
        .await?;
        let testbed_id =
            QueryTestbed::get_or_create(context, project_id, &json_report.testbed).await?;

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

        // If there is a hash then try to see if there is already a code version for
        // this branch with that particular hash.
        // Otherwise, create a new code version for this branch with/without the hash.
        let version_id = QueryVersion::get_or_increment(
            write_conn!(context),
            project_id,
            head_id,
            json_report.hash.as_ref(),
        )?;

        let json_settings = json_report.settings.take().unwrap_or_default();
        let adapter = json_settings.adapter.unwrap_or_default();

        // Create a new report and add it to the database
        let insert_report = InsertReport::from_json(
            public_user.user_id(),
            project_id,
            head_id,
            version_id,
            testbed_id,
            &json_report,
            adapter,
        );

        diesel::insert_into(schema::report::table)
            .values(&insert_report)
            .execute(write_conn!(context))
            .map_err(resource_conflict_err!(Report, insert_report))?;

        let query_report = schema::report::table
        .filter(schema::report::uuid.eq(&insert_report.uuid))
        .first::<QueryReport>(public_conn!(context, public_user))
        .map_err(|e| {
            issue_error(
                "Failed to find new report that was just created",
                &format!("Failed to find new report ({insert_report:?}) in project ({project_id}) on Bencher even though it was just created."),
                e,
            )
        })?;

        #[cfg(feature = "plus")]
        let mut usage = 0;

        // Process and record the report results
        #[cfg(feature = "plus")]
        let spec_id =
            QueryJob::spec_id_for_report_id(public_conn!(context, public_user), query_report.id)?;
        let mut report_results = ReportResults::new(
            project_id,
            branch_id,
            head_id,
            testbed_id,
            #[cfg(feature = "plus")]
            spec_id,
            query_report.id,
        );
        let results_array: Vec<&str> = json_report.results.iter().map(AsRef::as_ref).collect();
        let processed_report = report_results
            .process(
                log,
                context,
                &results_array,
                adapter,
                json_settings,
                #[cfg(feature = "plus")]
                &mut usage,
            )
            .await;

        #[cfg(feature = "plus")]
        plan_kind
            .check_usage(context.biller.as_ref(), query_project, usage)
            .await?;

        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::ReportCreate);

        // Don't return the error from processing the report until after the metrics usage has been checked
        processed_report?;
        // If the report was processed successfully, then return the report with the results
        query_report.into_json(log, public_conn!(context, public_user))
    }

    pub fn into_json(self, log: &Logger, conn: &mut DbConnection) -> Result<JsonReport, HttpError> {
        let Self {
            id,
            uuid,
            user_id,
            project_id,
            head_id,
            version_id,
            testbed_id,
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
        #[cfg(feature = "plus")]
        let spec_id = QueryJob::spec_id_for_report_id(conn, id)?;
        let testbed = QueryTestbed::get_json_for_report(
            conn,
            &query_project,
            testbed_id,
            #[cfg(feature = "plus")]
            spec_id,
        )?;
        let results = get_report_results(log, conn, &query_project, id)?;
        let alerts = get_report_alerts(conn, &query_project, id, head_id, version_id)?;

        let project = query_project.into_json(conn)?;
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
            created,
        })
    }
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

fn get_report_alerts(
    conn: &mut DbConnection,
    project: &QueryProject,
    report_id: ReportId,
    head_id: HeadId,
    version_id: VersionId,
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
    pub user_id: Option<UserId>,
    pub project_id: ProjectId,
    pub head_id: HeadId,
    pub version_id: VersionId,
    pub testbed_id: TestbedId,
    pub adapter: Adapter,
    pub start_time: DateTime,
    pub end_time: DateTime,
    pub created: DateTime,
}

impl InsertReport {
    #[cfg(feature = "plus")]
    crate::macros::rate_limit::fn_rate_limit!(report, Report);

    pub fn from_json(
        user_id: Option<UserId>,
        project_id: ProjectId,
        head_id: HeadId,
        version_id: VersionId,
        testbed_id: TestbedId,
        report: &JsonNewReport,
        adapter: Adapter,
    ) -> Self {
        Self {
            uuid: ReportUuid::new(),
            user_id,
            project_id,
            head_id,
            version_id,
            testbed_id,
            adapter,
            start_time: report.start_time,
            end_time: report.end_time,
            created: DateTime::now(),
        }
    }
}
