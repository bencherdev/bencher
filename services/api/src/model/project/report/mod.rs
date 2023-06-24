use std::{collections::HashMap, str::FromStr};

use bencher_json::{
    project::{
        benchmark::JsonBenchmarkMetric,
        report::{
            JsonAdapter, JsonReportAlerts, JsonReportIteration, JsonReportResult,
            JsonReportResults, JsonReportThreshold,
        },
    },
    BenchmarkName, JsonBenchmark, JsonMetricKind, JsonNewReport, JsonReport,
};
use chrono::{DateTime, TimeZone, Utc};
use diesel::{ExpressionMethods, Insertable, JoinOnDsl, QueryDsl, Queryable, RunQueryDsl};
use uuid::Uuid;

use self::adapter::Adapter;

use super::{
    branch::QueryBranch,
    metric::QueryMetric,
    metric_kind::QueryMetricKind,
    testbed::QueryTestbed,
    threshold::{
        alert::QueryAlert, boundary::QueryBoundary, statistic::QueryStatistic, QueryThreshold,
    },
    QueryProject,
};
use crate::{
    context::DbConnection,
    error::api_error,
    model::{
        project::{benchmark::QueryBenchmark, perf::QueryPerf},
        user::QueryUser,
    },
    schema,
    schema::report as report_table,
    util::{error::database_map, query::fn_get_id},
    ApiError,
};

mod adapter;
pub mod results;

#[derive(Queryable)]
pub struct QueryReport {
    pub id: i32,
    pub uuid: String,
    pub user_id: i32,
    pub branch_id: i32,
    pub version_id: i32,
    pub testbed_id: i32,
    pub adapter: i32,
    pub start_time: i64,
    pub end_time: i64,
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

    pub fn into_json(self, conn: &mut DbConnection) -> Result<JsonReport, ApiError> {
        let Self {
            uuid,
            user_id,
            adapter,
            start_time,
            end_time,
            ..
        } = self;

        let testbed = QueryTestbed::get(conn, self.testbed_id)?;
        Ok(JsonReport {
            uuid: Uuid::from_str(&uuid).map_err(api_error!())?,
            user: schema::user::table
                .filter(schema::user::id.eq(user_id))
                .first::<QueryUser>(conn)
                .map_err(api_error!())?
                .into_json()?,
            project: QueryProject::get(conn, testbed.project_id)?.into_json(conn)?,
            branch: QueryBranch::branch_version_json(conn, self.branch_id, self.version_id)?,
            testbed: testbed.into_json(conn)?,
            adapter: Adapter::try_from(adapter)?.into(),
            start_time: to_date_time(start_time)?,
            end_time: to_date_time(end_time)?,
            results: get_results(conn, self.id, self.branch_id, self.testbed_id)?,
            alerts: get_alerts(conn, self.id)?,
        })
    }
}

fn get_results(
    conn: &mut DbConnection,
    report_id: i32,
    branch_id: i32,
    testbed_id: i32,
) -> Result<JsonReportResults, ApiError> {
    let mut results = Vec::new();

    let mut iteration = 0;
    let mut metric_kinds = HashMap::new();
    let mut metric_kind_benchmarks = HashMap::<i32, Vec<JsonBenchmarkMetric>>::new();

    // Get the perfs for the report
    let perfs = get_perfs(conn, report_id)?;
    for perf in perfs {
        // Get the metric kinds
        metric_kinds = get_metric_kinds(conn, perf.id)?;

        // Create a stub benchmark metric to use for each metric kind
        let stub_benchmark_metric = get_stub_benchmark_metric(conn, perf.benchmark_id)?;

        // If the iteration is the same as the previous one, add the benchmark to the benchmarks list for all metric kinds
        // Otherwise, create a new iteration result and add it to the results list
        // Then add the benchmark to a new benchmarks list for all metric kinds
        if perf.iteration == iteration {
            for metric_kind_id in metric_kinds.keys().cloned() {
                let benchmark_metric = get_benchmark_metric(
                    conn,
                    perf.id,
                    metric_kind_id,
                    stub_benchmark_metric.clone(),
                )?;
                if let Some(benchmarks) = metric_kind_benchmarks.get_mut(&metric_kind_id) {
                    benchmarks.push(benchmark_metric);
                } else {
                    metric_kind_benchmarks.insert(metric_kind_id, vec![benchmark_metric]);
                }
            }
        } else {
            let iteration_results = get_iteration_results(
                conn,
                branch_id,
                testbed_id,
                &metric_kinds,
                std::mem::take(&mut metric_kind_benchmarks),
            )?;
            results.push(iteration_results);
            iteration = perf.iteration;
            for metric_kind_id in metric_kinds.keys().cloned() {
                let benchmark_metric = get_benchmark_metric(
                    conn,
                    perf.id,
                    metric_kind_id,
                    stub_benchmark_metric.clone(),
                )?;
                metric_kind_benchmarks.insert(metric_kind_id, vec![benchmark_metric]);
            }
        }
    }
    // Add the last iteration's metric kind and benchmark results
    let iteration_results = get_iteration_results(
        conn,
        branch_id,
        testbed_id,
        &metric_kinds,
        metric_kind_benchmarks,
    )?;
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
    // Then ordering by the benchmark id makes sure that the benchmarks are in the same order for each iteration
    .order((schema::perf::iteration,schema::benchmark::name))
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
    perf_id: i32,
) -> Result<HashMap<i32, JsonMetricKind>, ApiError> {
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
        ))
        .load::<QueryMetricKind>(conn)
        .map_err(api_error!())?
        .into_iter()
        .filter_map(|metric_kind| Some((metric_kind.id, metric_kind.into_json(conn).ok()?)))
        .collect())
}

#[derive(Clone)]
struct StubBenchmarkMetric {
    uuid: Uuid,
    project: Uuid,
    name: BenchmarkName,
}

fn get_stub_benchmark_metric(
    conn: &mut DbConnection,
    benchmark_id: i32,
) -> Result<StubBenchmarkMetric, ApiError> {
    let json_benchmark = schema::benchmark::table
        .filter(schema::benchmark::id.eq(benchmark_id))
        .first::<QueryBenchmark>(conn)
        .map_err(api_error!())?
        .into_json(conn)?;
    let JsonBenchmark {
        uuid,
        project,
        name,
    } = json_benchmark;
    Ok(StubBenchmarkMetric {
        uuid,
        project,
        name,
    })
}

fn get_benchmark_metric(
    conn: &mut DbConnection,
    perf_id: i32,
    metric_kind_id: i32,
    stub_benchmark_metric: StubBenchmarkMetric,
) -> Result<JsonBenchmarkMetric, ApiError> {
    let metric = schema::metric::table
        .filter(schema::metric::perf_id.eq(perf_id))
        .filter(schema::metric::metric_kind_id.eq(metric_kind_id))
        .first::<QueryMetric>(conn)
        .map_err(api_error!())?;
    let boundary = QueryBoundary::json_boundary(conn, metric.id);

    let StubBenchmarkMetric {
        uuid,
        project,
        name,
    } = stub_benchmark_metric;

    Ok(JsonBenchmarkMetric {
        uuid,
        project,
        name,
        metric: metric.into_json(),
        boundary,
    })
}

fn get_iteration_results(
    conn: &mut DbConnection,
    branch_id: i32,
    testbed_id: i32,
    metric_kinds: &HashMap<i32, JsonMetricKind>,
    metric_kind_benchmarks: HashMap<i32, Vec<JsonBenchmarkMetric>>,
) -> Result<JsonReportIteration, ApiError> {
    let mut iteration_results = Vec::new();
    for (metric_kind_id, benchmarks) in metric_kind_benchmarks {
        let Some(metric_kind) = metric_kinds.get(&metric_kind_id).cloned() else {
            tracing::warn!("Metric kind {metric_kind_id} not found in metric kinds list");
            continue;
        };

        let threshold = if let Ok(threshold) = schema::threshold::table
            .filter(schema::threshold::metric_kind_id.eq(metric_kind_id))
            .filter(schema::threshold::branch_id.eq(branch_id))
            .filter(schema::threshold::testbed_id.eq(testbed_id))
            .first::<QueryThreshold>(conn)
        {
            Some(JsonReportThreshold {
                uuid: Uuid::from_str(&threshold.uuid).map_err(api_error!())?,
                statistic: QueryStatistic::get(conn, threshold.statistic_id)?.into_json()?,
            })
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
        .order(schema::alert::id)
        .select((
            schema::alert::id,
            schema::alert::uuid,
            schema::alert::boundary_id,
            schema::alert::side,
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

// https://docs.rs/chrono/latest/chrono/serde/ts_nanoseconds/index.html
#[allow(
    clippy::cast_sign_loss,
    clippy::integer_division,
    clippy::modulo_arithmetic
)]
pub fn to_date_time(timestamp: i64) -> Result<DateTime<Utc>, ApiError> {
    Utc.timestamp_opt(
        timestamp / 1_000_000_000,
        (timestamp % 1_000_000_000) as u32,
    )
    .single()
    .ok_or(ApiError::Timestamp(timestamp))
}

#[derive(Insertable)]
#[diesel(table_name = report_table)]
pub struct InsertReport {
    pub uuid: String,
    pub user_id: i32,
    pub branch_id: i32,
    pub version_id: i32,
    pub testbed_id: i32,
    pub adapter: i32,
    pub start_time: i64,
    pub end_time: i64,
}

impl InsertReport {
    pub fn from_json(
        user_id: i32,
        branch_id: i32,
        version_id: i32,
        testbed_id: i32,
        report: &JsonNewReport,
        adapter: JsonAdapter,
    ) -> Self {
        Self {
            uuid: Uuid::new_v4().to_string(),
            user_id,
            branch_id,
            version_id,
            testbed_id,
            adapter: Adapter::from(adapter) as i32,
            start_time: report.start_time.timestamp_nanos(),
            end_time: report.end_time.timestamp_nanos(),
        }
    }
}
