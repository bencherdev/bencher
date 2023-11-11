use bencher_json::{
    project::{
        boundary::JsonBoundary,
        branch::{JsonVersion, VersionNumber},
        perf::{Iteration, JsonPerfMetric, JsonPerfMetrics, JsonPerfQueryParams},
    },
    BenchmarkUuid, BranchUuid, DateTime, GitHash, JsonBenchmark, JsonBranch, JsonPerf,
    JsonPerfQuery, JsonTestbed, ReportUuid, ResourceId, TestbedUuid,
};
use diesel::{
    sql_types::Json, CombineDsl, ExpressionMethods, NullableExpressionMethods, QueryDsl,
    RunQueryDsl, SelectableHelper,
};
use dropshot::{endpoint, HttpError, Path, Query, RequestContext};
use http::StatusCode;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    context::{ApiContext, DbConnection},
    endpoints::{
        endpoint::{CorsResponse, Get, ResponseOk},
        Endpoint,
    },
    error::{bad_request_error, issue_error, resource_not_found_err},
    model::user::auth::AuthUser,
    model::{
        project::{
            benchmark::{BenchmarkId, QueryBenchmark},
            branch::{BranchId, QueryBranch},
            metric::QueryMetric,
            metric_kind::{MetricKindId, QueryMetricKind},
            testbed::{QueryTestbed, TestbedId},
            threshold::{
                alert::QueryAlert, boundary::QueryBoundary, statistic::QueryStatistic,
                QueryThreshold,
            },
            QueryProject,
        },
        user::auth::PubBearerToken,
    },
    schema,
};

pub mod img;
mod macros;

use macros::{MetricsQuery, MAX_PERMUTATIONS};

#[derive(Deserialize, JsonSchema)]
pub struct ProjPerfParams {
    pub project: ResourceId,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/perf",
    tags = ["projects", "perf"]
}]
pub async fn proj_perf_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjPerfParams>,
    _query_params: Query<JsonPerfQueryParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/perf",
    tags = ["projects", "perf"]
}]
pub async fn proj_perf_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubBearerToken,
    path_params: Path<ProjPerfParams>,
    query_params: Query<JsonPerfQueryParams>,
) -> Result<ResponseOk<JsonPerf>, HttpError> {
    // Second round of marshaling
    let json_perf_query = query_params
        .into_inner()
        .try_into()
        .map_err(bad_request_error)?;

    let auth_user = AuthUser::from_pub_token(rqctx.context(), bearer_token).await?;
    let json = get_inner(
        rqctx.context(),
        path_params.into_inner(),
        json_perf_query,
        auth_user.as_ref(),
    )
    .await?;
    Ok(Get::response_ok(json, auth_user.is_some()))
}

async fn get_inner(
    context: &ApiContext,
    path_params: ProjPerfParams,
    json_perf_query: JsonPerfQuery,
    auth_user: Option<&AuthUser>,
) -> Result<JsonPerf, HttpError> {
    let conn = &mut *context.conn().await;

    let project =
        QueryProject::is_allowed_public(conn, &context.rbac, &path_params.project, auth_user)?;

    let JsonPerfQuery {
        branches,
        testbeds,
        benchmarks,
        metric_kind,
        start_time,
        end_time,
    } = json_perf_query;

    let metric_kind = QueryMetricKind::from_resource_id(conn, project.id, &metric_kind)?;

    let times = Times {
        start_time,
        end_time,
    };

    let mut permutations = Vec::with_capacity(branches.len() * testbeds.len() * benchmarks.len());
    for branch in branches {
        for testbed in &testbeds {
            for benchmark in &benchmarks {
                permutations.push((branch, *testbed, *benchmark));
            }
        }
    }

    let results = perf_query(conn, &project, metric_kind.id, &permutations, times);
    let metric_kind = metric_kind.into_json_for_project(&project);

    Ok(JsonPerf {
        project: project.into_json(conn)?,
        metric_kind,
        start_time,
        end_time,
        results,
    })
}

#[derive(Clone, Copy)]
struct Times {
    start_time: Option<DateTime>,
    end_time: Option<DateTime>,
}

type PerfQuery = (
    QueryBranch,
    QueryTestbed,
    QueryBenchmark,
    ReportUuid,
    Iteration,
    DateTime,
    DateTime,
    VersionNumber,
    Option<GitHash>,
    Option<(
        QueryBoundary,
        QueryThreshold,
        QueryStatistic,
        Option<QueryAlert>,
    )>,
    QueryMetric,
);

struct QueryDimensions {
    branch: QueryBranch,
    testbed: QueryTestbed,
    benchmark: QueryBenchmark,
}

type PerfMetricQuery = (
    ReportUuid,
    Iteration,
    DateTime,
    DateTime,
    VersionNumber,
    Option<GitHash>,
    Option<(
        QueryBoundary,
        QueryThreshold,
        QueryStatistic,
        Option<QueryAlert>,
    )>,
    QueryMetric,
);

fn from_perf_query(
    (
        branch,
        testbed,
        benchmark,
        report_uuid,
        iteration,
        start_time,
        end_time,
        version_number,
        version_hash,
        boundary_limit,
        query_metric,
    ): PerfQuery,
) -> (QueryDimensions, PerfMetricQuery) {
    let query_dimensions = QueryDimensions {
        branch,
        testbed,
        benchmark,
    };
    let metric_query = (
        report_uuid,
        iteration,
        start_time,
        end_time,
        version_number,
        version_hash,
        boundary_limit,
        query_metric,
    );
    (query_dimensions, metric_query)
}

#[allow(clippy::too_many_lines)]
fn perf_query(
    conn: &mut DbConnection,
    project: &QueryProject,
    metric_kind_id: MetricKindId,
    permutations: &[(BranchUuid, TestbedUuid, BenchmarkUuid)],
    times: Times,
) -> Vec<JsonPerfMetrics> {
    permutations
        .into_iter()
        .filter_map(|(branch_uuid, testbed_uuid, benchmark_uuid)| {
            let mut query = schema::metric::table
                .filter(schema::metric::metric_kind_id.eq(metric_kind_id))
                .inner_join(
                    schema::perf::table.inner_join(
                        schema::report::table
                            .inner_join(schema::version::table
                                .inner_join(schema::branch_version::table
                                    .inner_join(schema::branch::table)
                                ),
                            )
                            .inner_join(schema::testbed::table)
                    )
                    .inner_join(schema::benchmark::table)
                )
                // It is important to filter for the branch on the `branch_version` table and not on the branch in the `report` table.
                // This is because the `branch_version` table is the one that is updated when a branch is cloned/used as a start point.
                // In contrast, the `report` table is only set to a single branch when the report is created.
                .filter(schema::branch::uuid.eq(branch_uuid))
                .filter(schema::testbed::uuid.eq(testbed_uuid))
                .filter(schema::benchmark::uuid.eq(benchmark_uuid))
                // There may or may not be a boundary for any given metric
                .left_join(
                    schema::boundary::table
                        .inner_join(schema::threshold::table)
                        .inner_join(schema::statistic::table)
                        // There may or may not be an alert for any given boundary
                        .left_join(schema::alert::table),
                )
                .into_boxed();

            let Times {
                start_time,
                end_time,
            } = times;

            if let Some(start_time) = start_time {
                query = query.filter(schema::report::start_time.ge(start_time));
            }
            if let Some(end_time) = end_time {
                query = query.filter(schema::report::end_time.le(end_time));
            }

            query
                // Order by the version number so that the oldest version is first.
                // Because multiple reports can use the same version (via git hash), order by the start time next.
                // Then within a report order by the iteration number.
                .order((
                    schema::version::number,
                    schema::report::start_time,
                    schema::perf::iteration,
                ))
                .select((
                QueryBranch::as_select(),
                QueryTestbed::as_select(),
                QueryBenchmark::as_select(),
                schema::report::uuid,
                schema::perf::iteration,
                schema::report::start_time,
                schema::report::end_time,
                schema::version::number,
                schema::version::hash,
                (
                    (
                        schema::boundary::id,
                        schema::boundary::uuid,
                        schema::boundary::threshold_id,
                        schema::boundary::statistic_id,
                        schema::boundary::metric_id,
                        schema::boundary::lower_limit,
                        schema::boundary::upper_limit,
                    ),
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
                    ),
                    (
                        schema::alert::id,
                        schema::alert::uuid,
                        schema::alert::boundary_id,
                        schema::alert::boundary_limit,
                        schema::alert::status,
                        schema::alert::modified,
                    )
                        .nullable(),
                )
                    .nullable(),
                    QueryMetric::as_select(),
                ))
                .load::<PerfQuery>(conn)
                .ok()?
                .into_iter()
                .fold(None, |  perf_metrics: Option<JsonPerfMetrics>, perf_query| {
                    let (query_dimensions, query) = from_perf_query(perf_query);
                    let perf_metric = into_perf_metric(project, query);
                    if let Some(mut perf_metrics) = perf_metrics {
                        perf_metrics.metrics.push(perf_metric);
                        Some(perf_metrics)
                    } else {
                        Some(new_perf_metrics(project, query_dimensions, perf_metric))
                    }
                })
        })
        .fold(
            Vec::new(),
            |mut results: Vec<JsonPerfMetrics>, perf_metrics| {
                results.push(perf_metrics);
                results
            },
        )
}

// fn into_perf_metrics(
//     project: &QueryProject,
//     mut results: Vec<JsonPerfMetrics>,
//     perf_metrics: Option<JsonPerfMetrics>,
//     query: Vec<PerfQuery>,
// ) -> (Vec<JsonPerfMetrics>, Option<JsonPerfMetrics>) {
//     let (query_dimensions, query) = from_perf_query(query);
//     let metric = into_perf_metric(project, query);
//     let perf_metrics = if let Some(mut perf_metrics) = perf_metrics {
//         if query_dimensions.branch.uuid == perf_metrics.branch.uuid
//             && query_dimensions.testbed.uuid == perf_metrics.testbed.uuid
//             && query_dimensions.benchmark.uuid == perf_metrics.benchmark.uuid
//         {
//             perf_metrics.metrics.push(metric);
//             perf_metrics
//         } else {
//             results.push(perf_metrics);
//             new_perf_metrics(project, query_dimensions, metric)
//         }
//     } else {
//         new_perf_metrics(project, query_dimensions, metric)
//     };
//     (results, Some(perf_metrics))
// }

fn new_perf_metrics(
    project: &QueryProject,
    query_dimensions: QueryDimensions,
    metric: JsonPerfMetric,
) -> JsonPerfMetrics {
    let QueryDimensions {
        branch,
        testbed,
        benchmark,
    } = query_dimensions;
    JsonPerfMetrics {
        branch: branch.into_json_for_project(project),
        testbed: testbed.into_json_for_project(project),
        benchmark: benchmark.into_json_for_project(project),
        metrics: vec![metric],
    }
}

fn into_perf_metric(
    project: &QueryProject,
    (
        report_uuid,
        iteration,
        start_time,
        end_time,
        version_number,
        version_hash,
        boundary_limit,
        query_metric,
    ): PerfMetricQuery,
) -> JsonPerfMetric {
    let version = JsonVersion {
        number: version_number,
        hash: version_hash,
    };

    let (boundary, threshold, alert) =
        if let Some((query_boundary, query_threshold, query_statistic, query_alert)) =
            boundary_limit
        {
            let boundary = query_boundary.into_json();
            let threshold = Some(
                query_threshold.into_threshold_statistic_json_for_project(project, query_statistic),
            );
            let alert = query_alert.map(QueryAlert::into_perf_json);
            (boundary, threshold, alert)
        } else {
            (JsonBoundary::default(), None, None)
        };

    JsonPerfMetric {
        report: report_uuid,
        iteration,
        start_time,
        end_time,
        version,
        threshold,
        metric: query_metric.into_json(),
        boundary,
        alert,
    }
}
