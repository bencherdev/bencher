use std::{collections::HashMap, str::FromStr};

use bencher_json::{
    project::{
        benchmark::JsonBenchmarkMetric,
        report::{
            JsonAdapter, JsonReportAlerts, JsonReportIteration, JsonReportResult, JsonReportResults,
        },
    },
    JsonBenchmark, JsonMetricKind, JsonNewReport, JsonReport,
};
use chrono::Utc;
use diesel::{
    ExpressionMethods, JoinOnDsl, NullableExpressionMethods, QueryDsl, RunQueryDsl,
    SelectableHelper,
};
use mail_send::mail_auth::trust_dns_resolver::proto::op::query;
use slog::{warn, Logger};
use uuid::Uuid;

use self::adapter::Adapter;

use super::{
    branch::{BranchId, QueryBranch},
    metric::QueryMetric,
    metric_kind::{MetricKindId, QueryMetricKind},
    perf::PerfId,
    testbed::{QueryTestbed, TestbedId},
    threshold::{
        alert::QueryAlert, boundary::QueryBoundary, statistic::StatisticId, QueryThreshold,
        ThresholdId,
    },
    version::VersionId,
    ProjectId, QueryProject,
};
use crate::{
    context::DbConnection,
    model::{
        project::{
            benchmark::QueryBenchmark, perf::QueryPerf, threshold::statistic::QueryStatistic,
        },
        user::{QueryUser, UserId},
    },
    schema,
    schema::report as report_table,
    util::{error::database_map, query::fn_get_id, to_date_time},
    ApiError,
};

mod adapter;
pub mod results;

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
        get_report_results(log, conn, project.uuid, id)?;

        Ok(JsonReport {
            uuid: Uuid::from_str(&uuid).map_err(ApiError::from)?,
            user: QueryUser::get(conn, user_id)?.into_json()?,
            project: QueryProject::get(conn, project_id)?.into_json(conn)?,
            branch: QueryBranch::get_branch_version_json(conn, branch_id, version_id)?,
            testbed: QueryTestbed::get(conn, testbed_id)?.into_json(conn)?,
            adapter: adapter.into(),
            start_time: to_date_time(start_time)?,
            end_time: to_date_time(end_time)?,
            results: get_results(log, conn, id)?,
            alerts: get_alerts(conn, id)?,
            created: to_date_time(created).map_err(ApiError::from)?,
        })
    }
}

#[allow(clippy::too_many_lines)]
fn get_report_results(
    log: &Logger,
    conn: &mut DbConnection,
    project: Uuid,
    report_id: ReportId,
) -> Result<JsonReportResults, ApiError> {
    let results = schema::perf::table
    .filter(schema::perf::report_id.eq(report_id))
    .inner_join(
        schema::benchmark::table.on(schema::perf::benchmark_id.eq(schema::benchmark::id)),
    )
    // It is important to order by the iteration first in order to make sure they are grouped together below
    // Then ordering by the benchmark name makes sure that the benchmarks are in the same order for each iteration
    .inner_join(
        schema::metric::table.on(schema::perf::id.eq(schema::metric::perf_id)),
    )
    .inner_join(
        schema::metric_kind::table.on(schema::metric::metric_kind_id.eq(schema::metric_kind::id)),
    )
    .left_join(schema::boundary::table.on(schema::metric::id.eq(schema::boundary::metric_id)).inner_join(
        schema::threshold::table.on(schema::boundary::threshold_id.eq(schema::threshold::id))
        ).inner_join(schema::statistic::table.on(schema::boundary::statistic_id.eq(schema::statistic::id))),
    )
    .order((schema::perf::iteration, schema::metric_kind::name, schema::benchmark::name))
    .select((
        schema::perf::iteration,
        (
            schema::metric_kind::id,
            schema::metric_kind::uuid,
            schema::metric_kind::project_id,
            schema::metric_kind::name,
            schema::metric_kind::slug,
            schema::metric_kind::units,
            schema::metric_kind::created,
            schema::metric_kind::modified
        ),

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

        (
            schema::benchmark::id,
            schema::benchmark::uuid,
            schema::benchmark::project_id,
            schema::benchmark::name,
            schema::benchmark::slug,
            schema::benchmark::created,
            schema::benchmark::modified,
        ),

        // schema::metric::id,
        // schema::metric::uuid,
        // schema::metric::value,
        // schema::metric::lower_value,
        // schema::metric::upper_value,

        // (
        //     schema::boundary::lower_limit,
        //     schema::boundary::upper_limit,
        // ).nullable(),
    ))
    .load::<(i32, QueryMetricKind, Option<(QueryThreshold, QueryStatistic)>, QueryBenchmark)>(conn)
    .map_err(ApiError::from)?;

    let mut report_results = Vec::new();
    let mut report_iteration = Vec::new();
    let mut iteration = 0;
    let mut report_result: Option<JsonReportResult> = None;
    for (i, query_metric_kind, threshold_statistic, query_benchmark) in results {
        // If onto a new iteration, then add the previous iteration's results to the report results list.
        if i != iteration {
            iteration = i;
            report_results.push(report_iteration);
            report_iteration = Vec::new();
        }

        // If there is a current report result, make sure that the metric kind is the same.
        // Otherwise, add it to the report iteration list.
        if let Some(result) = report_result.take() {
            if query_metric_kind.uuid == result.metric_kind.uuid.to_string() {
                slog::trace!(
                    log,
                    "Same metric kind {} | {}",
                    query_metric_kind.uuid,
                    result.metric_kind.uuid
                );
                report_result = Some(result);
            } else {
                slog::trace!(
                    log,
                    "Different metric kind {} | {}",
                    query_metric_kind.uuid,
                    result.metric_kind.uuid
                );
                report_iteration.push(result);
            }
        }

        let benchmark = query_benchmark.into_json_for_project(project)?;

        if let Some(result) = report_result.as_mut() {
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
                benchmarks: Vec::new(),
            });
        }
    }

    // Save from the last iteration
    if let Some(result) = report_result.take() {
        report_iteration.push(result);
    }
    report_results.push(report_iteration);

    slog::debug!(log, "Report results: {report_results:#?}");

    Ok(report_results)
}

fn get_results(
    log: &Logger,
    conn: &mut DbConnection,
    report_id: ReportId,
) -> Result<JsonReportResults, ApiError> {
    let mut results = Vec::new();

    let mut iteration = 0;
    let mut metric_kinds = HashMap::new();
    let mut metric_kind_benchmarks =
        HashMap::<MetricKindId, (Option<ThresholdStatistic>, Vec<JsonBenchmarkMetric>)>::new();

    // Get the perfs for the report
    let perfs = get_perfs(conn, report_id)?;
    for perf in perfs {
        // Get the metric kinds
        metric_kinds = get_metric_kinds(conn, perf.id)?;

        // Get the benchmark to use for each metric kind
        let benchmark = QueryBenchmark::get(conn, perf.benchmark_id)?.into_json(conn)?;

        // If the iteration is the same as the previous one, add the benchmark to the benchmarks list for all metric kinds
        // Otherwise, create a new iteration result and add it to the results list
        // Then add the benchmark to a new benchmarks list for all metric kinds
        // Only keep a single instance of the threshold statistic for each metric kind as it should be the same value for all benchmarks
        if perf.iteration == iteration {
            for metric_kind_id in metric_kinds.keys().copied() {
                let (threshold_statistic, benchmark_metric) =
                    get_benchmark_metric(conn, perf.id, metric_kind_id, benchmark.clone())?;
                if let Some((_, benchmarks)) = metric_kind_benchmarks.get_mut(&metric_kind_id) {
                    benchmarks.push(benchmark_metric);
                } else {
                    metric_kind_benchmarks.insert(
                        metric_kind_id,
                        (threshold_statistic, vec![benchmark_metric]),
                    );
                }
            }
        } else {
            let iteration_results = get_iteration_results(
                log,
                conn,
                &metric_kinds,
                std::mem::take(&mut metric_kind_benchmarks),
            )?;
            results.push(iteration_results);
            iteration = perf.iteration;
            for metric_kind_id in metric_kinds.keys().copied() {
                let (threshold_statistic, benchmark_metric) =
                    get_benchmark_metric(conn, perf.id, metric_kind_id, benchmark.clone())?;
                metric_kind_benchmarks.insert(
                    metric_kind_id,
                    (threshold_statistic, vec![benchmark_metric]),
                );
            }
        }
    }
    // Add the last iteration's metric kind and benchmark results
    let iteration_results =
        get_iteration_results(log, conn, &metric_kinds, metric_kind_benchmarks)?;
    results.push(iteration_results);

    Ok(results)
}

fn get_perfs(conn: &mut DbConnection, report_id: ReportId) -> Result<Vec<QueryPerf>, ApiError> {
    schema::perf::table
    .filter(schema::perf::report_id.eq(report_id))
    .inner_join(
        schema::benchmark::table.on(schema::perf::benchmark_id.eq(schema::benchmark::id)),
    )
    // It is important to order by the iteration first in order to make sure they are grouped together below
    // Then ordering by the benchmark name makes sure that the benchmarks are in the same order for each iteration
    .order((schema::perf::iteration, schema::benchmark::name))
    .select((
        schema::perf::id,
        schema::perf::uuid,
        schema::perf::report_id,
        schema::perf::iteration,
        schema::perf::benchmark_id,
    ))
    .load::<QueryPerf>(conn)
    .map_err(ApiError::from)
}

fn get_metric_kinds(
    conn: &mut DbConnection,
    perf_id: PerfId,
) -> Result<HashMap<MetricKindId, JsonMetricKind>, ApiError> {
    Ok(schema::metric_kind::table
        .left_join(
            schema::metric::table.on(schema::metric_kind::id.eq(schema::metric::metric_kind_id)),
        )
        .left_join(schema::perf::table.on(schema::metric::perf_id.eq(schema::perf::id)))
        .filter(schema::perf::id.eq(perf_id))
        .select((
            schema::metric_kind::id,
            schema::metric_kind::uuid,
            schema::metric_kind::project_id,
            schema::metric_kind::name,
            schema::metric_kind::slug,
            schema::metric_kind::units,
            schema::metric_kind::created,
            schema::metric_kind::modified,
        ))
        .load::<QueryMetricKind>(conn)
        .map_err(ApiError::from)?
        .into_iter()
        .filter_map(|metric_kind| Some((metric_kind.id, metric_kind.into_json(conn).ok()?)))
        .collect())
}

struct ThresholdStatistic {
    threshold_id: ThresholdId,
    statistic_id: StatisticId,
}

fn get_benchmark_metric(
    conn: &mut DbConnection,
    perf_id: PerfId,
    metric_kind_id: MetricKindId,
    benchmark: JsonBenchmark,
) -> Result<(Option<ThresholdStatistic>, JsonBenchmarkMetric), ApiError> {
    let query_metric = schema::metric::table
        .filter(schema::metric::perf_id.eq(perf_id))
        .filter(schema::metric::metric_kind_id.eq(metric_kind_id))
        .first::<QueryMetric>(conn)
        .map_err(ApiError::from)?;
    // The boundary is optional, so it may not exist
    let query_boundary = QueryBoundary::from_metric_id(conn, query_metric.id).ok();

    let threshold_statistic = query_boundary.as_ref().map(
        |QueryBoundary {
             threshold_id,
             statistic_id,
             ..
         }| ThresholdStatistic {
            threshold_id: *threshold_id,
            statistic_id: *statistic_id,
        },
    );

    let JsonBenchmark {
        uuid,
        project,
        name,
        slug,
        created,
        modified,
    } = benchmark;

    Ok((
        threshold_statistic,
        JsonBenchmarkMetric {
            uuid,
            project,
            name,
            slug,
            metric: query_metric.into_json(),
            boundary: query_boundary
                .map(QueryBoundary::into_json)
                .unwrap_or_default(),
            created,
            modified,
        },
    ))
}

fn get_iteration_results(
    log: &Logger,
    conn: &mut DbConnection,
    metric_kinds: &HashMap<MetricKindId, JsonMetricKind>,
    metric_kind_benchmarks: HashMap<
        MetricKindId,
        (Option<ThresholdStatistic>, Vec<JsonBenchmarkMetric>),
    >,
) -> Result<JsonReportIteration, ApiError> {
    let mut iteration_results = Vec::new();
    for (metric_kind_id, (threshold_statistic, benchmarks)) in metric_kind_benchmarks {
        let Some(metric_kind) = metric_kinds.get(&metric_kind_id).cloned() else {
            warn!(
                log,
                "Metric kind {metric_kind_id} not found in metric kinds list"
            );
            continue;
        };

        let threshold = if let Some(ThresholdStatistic {
            threshold_id,
            statistic_id,
        }) = threshold_statistic
        {
            Some(QueryThreshold::get_threshold_statistic_json(
                conn,
                threshold_id,
                statistic_id,
            )?)
        } else {
            None
        };

        let result = JsonReportResult {
            metric_kind,
            threshold,
            benchmarks,
        };
        iteration_results.push(result);
    }
    Ok(iteration_results)
}

fn get_alerts(conn: &mut DbConnection, report_id: ReportId) -> Result<JsonReportAlerts, ApiError> {
    Ok(schema::alert::table
        .left_join(schema::boundary::table.on(schema::alert::boundary_id.eq(schema::boundary::id)))
        .left_join(schema::metric::table.on(schema::metric::id.eq(schema::boundary::metric_id)))
        .left_join(schema::perf::table.on(schema::metric::perf_id.eq(schema::perf::id)))
        .filter(schema::perf::report_id.eq(report_id))
        .left_join(
            schema::benchmark::table.on(schema::perf::benchmark_id.eq(schema::benchmark::id)),
        )
        .order((schema::benchmark::name.asc(), schema::perf::iteration.asc()))
        .select((
            schema::alert::id,
            schema::alert::uuid,
            schema::alert::boundary_id,
            schema::alert::boundary_limit,
            schema::alert::status,
            schema::alert::modified,
        ))
        .load::<QueryAlert>(conn)
        .map_err(ApiError::from)?
        .into_iter()
        .filter_map(|alert| {
            database_map("QueryReport::get_alerts", alert.into_json(conn)).map(Into::into)
        })
        .collect())
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
