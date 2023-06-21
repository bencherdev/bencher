use std::{collections::HashMap, str::FromStr};

use bencher_json::{
    project::{
        benchmark::JsonBenchmarkMetric,
        report::{
            JsonAdapter, JsonReportAlerts, JsonReportIteration, JsonReportResult, JsonReportResults,
        },
    },
    JsonBenchmark, JsonMetric, JsonMetricKind, JsonNewReport, JsonPerfQuery, JsonReport,
    ResourceId, Url,
};
use chrono::{DateTime, TimeZone, Utc};
use diesel::{ExpressionMethods, Insertable, JoinOnDsl, QueryDsl, Queryable, RunQueryDsl};
use uuid::Uuid;

use self::adapter::Adapter;

use super::{
    branch::QueryBranch, metric::QueryMetric, metric_kind::QueryMetricKind, testbed::QueryTestbed,
    visibility::Visibility, QueryProject,
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

    pub fn into_json(
        self,
        conn: &mut DbConnection,
        endpoint: &url::Url,
        project: &QueryProject,
    ) -> Result<JsonReport, ApiError> {
        let branch = QueryBranch::branch_version_json(conn, self.branch_id, self.version_id)?;
        let testbed = schema::testbed::table
            .filter(schema::testbed::id.eq(self.testbed_id))
            .first::<QueryTestbed>(conn)
            .map_err(api_error!())?
            .into_json(conn)?;

        let results = self.get_results(conn, endpoint, project, branch.uuid, testbed.uuid)?;
        let alerts = self.get_alerts(conn)?;

        let Self {
            uuid,
            user_id,
            adapter,
            start_time,
            end_time,
            ..
        } = self;

        let user = schema::user::table
            .filter(schema::user::id.eq(user_id))
            .first::<QueryUser>(conn)
            .map_err(api_error!())?
            .into_json()?;

        Ok(JsonReport {
            uuid: Uuid::from_str(&uuid).map_err(api_error!())?,
            user,
            branch,
            testbed,
            adapter: Adapter::try_from(adapter)?.into(),
            start_time: to_date_time(start_time)?,
            end_time: to_date_time(end_time)?,
            results,
            alerts,
        })
    }

    fn get_results(
        &self,
        conn: &mut DbConnection,
        endpoint: &url::Url,
        project: &QueryProject,
        branch: Uuid,
        testbed: Uuid,
    ) -> Result<JsonReportResults, ApiError> {
        let mut results = Vec::new();

        let mut iteration = 0;
        let mut metric_kinds = HashMap::new();
        let mut metric_kind_benchmarks = HashMap::<i32, Vec<JsonBenchmarkMetric>>::new();

        let perfs = get_perfs(conn, self.id)?;
        for perf in perfs {
            // Get the metric kinds
            metric_kinds = get_metric_kinds(conn, perf.id)?;

            // Create a default benchmark metric to use for each metric kind
            let default_benchmark_metric = get_default_benchmark_metric(conn, perf.benchmark_id)?;

            // If the iteration is the same as the previous one, add the benchmark to the benchmarks list for all metric kinds
            // Otherwise, create a new iteration result and add it to the results list
            // Then add the benchmark to a new benchmarks list for all metric kinds
            if perf.iteration == iteration {
                for metric_kind_id in metric_kinds.keys().cloned() {
                    let benchmark_metric = get_benchmark_metric(
                        conn,
                        perf.id,
                        metric_kind_id,
                        default_benchmark_metric.clone(),
                    )?;
                    if let Some(benchmarks) = metric_kind_benchmarks.get_mut(&metric_kind_id) {
                        benchmarks.push(benchmark_metric);
                    } else {
                        metric_kind_benchmarks.insert(metric_kind_id, vec![benchmark_metric]);
                    }
                }
            } else {
                let iteration_results = get_iteration_results(
                    endpoint,
                    project,
                    &metric_kinds,
                    branch,
                    testbed,
                    std::mem::take(&mut metric_kind_benchmarks),
                )?;
                results.push(iteration_results);
                iteration = perf.iteration;
                for metric_kind_id in metric_kinds.keys().cloned() {
                    let benchmark_metric = get_benchmark_metric(
                        conn,
                        perf.id,
                        metric_kind_id,
                        default_benchmark_metric.clone(),
                    )?;
                    metric_kind_benchmarks.insert(metric_kind_id, vec![benchmark_metric]);
                }
            }
        }
        // Add the last iteration's metric kind and benchmark results
        let iteration_results = get_iteration_results(
            endpoint,
            project,
            &metric_kinds,
            branch,
            testbed,
            metric_kind_benchmarks,
        )?;
        results.push(iteration_results);

        Ok(results)
    }

    fn get_alerts(&self, conn: &mut DbConnection) -> Result<JsonReportAlerts, ApiError> {
        Ok(schema::alert::table
            .left_join(schema::perf::table.on(schema::perf::id.eq(schema::alert::perf_id)))
            .filter(schema::perf::report_id.eq(self.id))
            .select(schema::alert::uuid)
            .order(schema::alert::id)
            .load::<String>(conn)
            .map_err(api_error!())?
            .iter()
            .filter_map(|uuid| {
                database_map("QueryReport::get_alerts", Uuid::from_str(uuid)).map(Into::into)
            })
            .collect())
    }
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

fn get_default_benchmark_metric(
    conn: &mut DbConnection,
    benchmark_id: i32,
) -> Result<JsonBenchmarkMetric, ApiError> {
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
    Ok(JsonBenchmarkMetric {
        uuid,
        project,
        name,
        metric: JsonMetric::default(),
    })
}

fn get_benchmark_metric(
    conn: &mut DbConnection,
    perf_id: i32,
    metric_kind_id: i32,
    mut benchmark_metric: JsonBenchmarkMetric,
) -> Result<JsonBenchmarkMetric, ApiError> {
    benchmark_metric.metric = schema::metric::table
        .filter(schema::metric::perf_id.eq(perf_id))
        .filter(schema::metric::metric_kind_id.eq(metric_kind_id))
        .first::<QueryMetric>(conn)
        .map_err(api_error!())?
        .into_json();
    Ok(benchmark_metric)
}

fn get_iteration_results(
    endpoint: &url::Url,
    project: &QueryProject,
    metric_kinds: &HashMap<i32, JsonMetricKind>,
    branch: Uuid,
    testbed: Uuid,
    metric_kind_benchmarks: HashMap<i32, Vec<JsonBenchmarkMetric>>,
) -> Result<JsonReportIteration, ApiError> {
    let mut iteration_results = Vec::new();
    for (metric_kind_id, benchmarks) in metric_kind_benchmarks {
        let Some(metric_kind) = metric_kinds.get(&metric_kind_id).cloned() else {
            tracing::warn!("Metric kind {metric_kind_id} not found in metric kinds list");
            continue;
        };
        let url = to_url(
            endpoint,
            project,
            metric_kind.uuid.into(),
            branch,
            testbed,
            benchmarks.iter().map(|benchmark| benchmark.uuid).collect(),
        )?;
        let result = JsonReportResult {
            metric_kind,
            benchmarks,
            url,
        };
        iteration_results.push(result);
    }
    Ok(iteration_results)
}

fn to_url(
    endpoint: &url::Url,
    project: &QueryProject,
    metric_kind: ResourceId,
    branch: Uuid,
    testbed: Uuid,
    benchmarks: Vec<Uuid>,
) -> Result<Url, ApiError> {
    let json_perf_query = JsonPerfQuery {
        metric_kind,
        branches: vec![branch],
        testbeds: vec![testbed],
        benchmarks,
        start_time: None,
        end_time: None,
    };

    let mut url = endpoint.clone();
    let path = match project.visibility()? {
        Visibility::Public => format!("/perf/{}", project.slug),
        Visibility::Private => format!("/console/projects/{}/perf", project.slug),
    };
    url.set_path(&path);
    url.set_query(Some(&json_perf_query.to_query_string(&[
            // ("tab", Some("benchmarks".into()))
            ])?));

    Ok(url.into())
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
