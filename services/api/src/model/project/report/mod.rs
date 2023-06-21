use std::{collections::HashMap, str::FromStr};

use bencher_json::{
    project::report::{JsonAdapter, JsonReportAlerts, JsonReportResult, JsonReportResults},
    JsonMetricKind, JsonNewReport, JsonPerfQuery, JsonReport,
};
use chrono::{DateTime, TimeZone, Utc};
use diesel::{ExpressionMethods, Insertable, JoinOnDsl, QueryDsl, Queryable, RunQueryDsl};
use uuid::Uuid;

use self::adapter::Adapter;

use super::{
    branch::QueryBranch, metric_kind::QueryMetricKind, testbed::QueryTestbed,
    version::QueryVersion, visibility::Visibility, QueryProject,
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
        let results = self.get_results(conn)?;
        let alerts = self.get_alerts(conn)?;
        let Self {
            uuid,
            user_id,
            branch_id,
            version_id,
            testbed_id,
            adapter,
            start_time,
            end_time,
            ..
        } = self;

        let branch = QueryBranch::get_uuid(conn, branch_id)?;
        let testbed = QueryTestbed::get_uuid(conn, testbed_id)?;
        let json_perf_query = JsonPerfQuery {
            metric_kind: "latency".parse().unwrap(),
            branches: vec![branch],
            testbeds: vec![testbed],
            benchmarks: vec![],
            start_time: None,
            end_time: None,
        };

        let mut url = endpoint.clone();
        let path = match project.visibility()? {
            Visibility::Public => format!("/perf/{}", project.slug),
            Visibility::Private => format!("/console/projects/{}/perf", project.slug),
        };
        url.set_path(&path);
        url.set_query(Some(
            &json_perf_query.to_query_string(&[("tab", Some("benchmarks".into()))])?,
        ));
        // let url = url.into();

        Ok(JsonReport {
            uuid: Uuid::from_str(&uuid).map_err(api_error!())?,
            user: QueryUser::get_uuid(conn, user_id)?,
            branch,
            version: QueryVersion::get_uuid(conn, version_id)?,
            testbed,
            adapter: Adapter::try_from(adapter)?.into(),
            start_time: to_date_time(start_time)?,
            end_time: to_date_time(end_time)?,
            results,
            alerts,
            // url,
        })
    }

    fn get_results(&self, conn: &mut DbConnection) -> Result<JsonReportResults, ApiError> {
        let perfs = schema::perf::table
            .filter(schema::perf::report_id.eq(self.id))
            // It is important to order by the iteration first in order to make sure they are grouped together below
            // Then ordering by the benchmark id makes sure that the benchmarks are in the same order for each iteration
            .order((schema::perf::iteration, schema::perf::benchmark_id))
            .load::<QueryPerf>(conn)
            .map_err(api_error!())?;

        let mut results = Vec::new();

        let mut iteration = 0;
        let mut metric_kinds = HashMap::<i32, JsonMetricKind>::new();
        let mut metric_kind_benchmarks = HashMap::<i32, Vec<Uuid>>::new();
        for perf in perfs {
            // Get the metric kinds
            metric_kinds = schema::metric_kind::table
                .left_join(
                    schema::metric::table
                        .on(schema::metric_kind::id.eq(schema::metric::metric_kind_id)),
                )
                .left_join(schema::perf::table.on(schema::metric::perf_id.eq(schema::perf::id)))
                .filter(schema::perf::id.eq(perf.id))
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
                .collect();
            // Get the UUID of the benchmark
            let benchmark = QueryBenchmark::get_uuid(conn, perf.benchmark_id)?;

            // If the iteration is the same as the previous one, add the benchmark to the benchmarks list for all metric kinds
            // Otherwise, create a new iteration result and add it to the results list
            // Then add the benchmark to a new benchmarks list for all metric kinds
            if perf.iteration == iteration {
                for metric_kind in metric_kinds.keys().cloned() {
                    if let Some(benchmarks) = metric_kind_benchmarks.get_mut(&metric_kind) {
                        benchmarks.push(benchmark);
                    } else {
                        metric_kind_benchmarks.insert(metric_kind, vec![benchmark]);
                    }
                }
            } else {
                let mut iteration_results = Vec::new();
                for (metric_kind, benchmarks) in std::mem::take(&mut metric_kind_benchmarks) {
                    let result = JsonReportResult {
                        metric_kind: if let Some(metric_kind) =
                            metric_kinds.get(&metric_kind).cloned()
                        {
                            metric_kind
                        } else {
                            continue;
                        },
                        benchmarks,
                        url: "http://example.com".parse().unwrap(),
                    };
                    iteration_results.push(result);
                }
                results.push(iteration_results);
                iteration = perf.iteration;
                for metric_kind in metric_kinds.keys().cloned() {
                    metric_kind_benchmarks.insert(metric_kind, vec![benchmark]);
                }
            }
        }
        // Add the last iteration's metric kind and benchmark results
        let mut iteration_results = Vec::new();
        for (metric_kind, benchmarks) in metric_kind_benchmarks {
            let result = JsonReportResult {
                metric_kind: if let Some(metric_kind) = metric_kinds.get(&metric_kind).cloned() {
                    metric_kind
                } else {
                    continue;
                },
                benchmarks,
                url: "http://example.com".parse().unwrap(),
            };
            iteration_results.push(result);
        }
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
