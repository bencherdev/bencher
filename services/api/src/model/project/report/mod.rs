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
use diesel::{ExpressionMethods, Insertable, JoinOnDsl, QueryDsl, Queryable, RunQueryDsl};
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
    error::api_error,
    model::{
        project::{benchmark::QueryBenchmark, perf::QueryPerf},
        user::{QueryUser, UserId},
    },
    schema,
    schema::report as report_table,
    util::{error::database_map, query::fn_get_id, to_date_time},
    ApiError,
};

mod adapter;
pub mod results;

#[derive(Queryable)]
pub struct QueryReport {
    pub id: i32,
    pub uuid: String,
    pub user_id: UserId,
    pub project_id: ProjectId,
    pub branch_id: BranchId,
    pub version_id: VersionId,
    pub testbed_id: TestbedId,
    pub adapter: i32,
    pub start_time: i64,
    pub end_time: i64,
    pub created: i64,
}

impl QueryReport {
    fn_get_id!(report);

    pub fn get_uuid(conn: &mut DbConnection, id: i32) -> Result<Uuid, ApiError> {
        let uuid: String = schema::report::table
            .filter(schema::report::id.eq(id))
            .select(schema::report::uuid)
            .first(conn)
            .map_err(api_error!())?;
        Uuid::from_str(&uuid).map_err(api_error!())
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

        Ok(JsonReport {
            uuid: Uuid::from_str(&uuid).map_err(api_error!())?,
            user: QueryUser::get(conn, user_id)?.into_json()?,
            project: QueryProject::get(conn, project_id)?.into_json(conn)?,
            branch: QueryBranch::get_branch_version_json(conn, branch_id, version_id)?,
            testbed: QueryTestbed::get(conn, testbed_id)?.into_json(conn)?,
            adapter: Adapter::try_from(adapter)?.into(),
            start_time: to_date_time(start_time)?,
            end_time: to_date_time(end_time)?,
            results: get_results(log, conn, id)?,
            alerts: get_alerts(conn, id)?,
            created: to_date_time(created).map_err(api_error!())?,
        })
    }
}

fn get_results(
    log: &Logger,
    conn: &mut DbConnection,
    report_id: i32,
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
            for metric_kind_id in metric_kinds.keys().cloned() {
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
            for metric_kind_id in metric_kinds.keys().cloned() {
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

fn get_perfs(conn: &mut DbConnection, report_id: i32) -> Result<Vec<QueryPerf>, ApiError> {
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
    .map_err(api_error!())
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
        .map_err(api_error!())?
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
        .map_err(api_error!())?;
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
            boundary: query_boundary.map(|b| b.into_json()).unwrap_or_default(),
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

fn get_alerts(conn: &mut DbConnection, report_id: i32) -> Result<JsonReportAlerts, ApiError> {
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
        .map_err(api_error!())?
        .into_iter()
        .filter_map(|alert| {
            database_map("QueryReport::get_alerts", alert.into_json(conn)).map(Into::into)
        })
        .collect())
}

#[derive(Insertable)]
#[diesel(table_name = report_table)]
pub struct InsertReport {
    pub uuid: String,
    pub user_id: UserId,
    pub project_id: ProjectId,
    pub branch_id: BranchId,
    pub version_id: VersionId,
    pub testbed_id: TestbedId,
    pub adapter: i32,
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
            adapter: Adapter::from(adapter) as i32,
            start_time: report.start_time.timestamp(),
            end_time: report.end_time.timestamp(),
            created: Utc::now().timestamp(),
        }
    }
}
