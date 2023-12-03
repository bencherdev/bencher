use bencher_json::{
    project::{
        boundary::JsonBoundary,
        branch::{JsonVersion, VersionNumber},
        perf::{Iteration, JsonPerfMetric, JsonPerfMetrics, JsonPerfQueryParams},
    },
    BenchmarkUuid, BranchUuid, DateTime, GitHash, JsonPerf, JsonPerfQuery, MetricKindUuid,
    ReportUuid, ResourceId, TestbedUuid,
};
use diesel::{
    ExpressionMethods, NullableExpressionMethods, QueryDsl, RunQueryDsl, SelectableHelper,
};
use dropshot::{endpoint, HttpError, Path, Query, RequestContext};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    context::{ApiContext, DbConnection},
    endpoints::{
        endpoint::{CorsResponse, Get, ResponseOk},
        Endpoint,
    },
    error::{bad_request_error, resource_not_found_err},
    model::user::auth::AuthUser,
    model::{
        project::{
            benchmark::QueryBenchmark,
            branch::QueryBranch,
            metric::QueryMetric,
            metric_kind::QueryMetricKind,
            testbed::QueryTestbed,
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

const MAX_PERMUTATIONS: usize = u8::MAX as usize;

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
        metric_kinds,
        branches,
        testbeds,
        benchmarks,
        start_time,
        end_time,
    } = json_perf_query;

    let times = Times {
        start_time,
        end_time,
    };

    let results = perf_results(
        conn,
        &project,
        &metric_kinds,
        &branches,
        &testbeds,
        &benchmarks,
        times,
    )?;

    Ok(JsonPerf {
        project: project.into_json(conn)?,
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

fn perf_results(
    conn: &mut DbConnection,
    project: &QueryProject,
    metric_kinds: &[MetricKindUuid],
    branches: &[BranchUuid],
    testbeds: &[TestbedUuid],
    benchmarks: &[BenchmarkUuid],
    times: Times,
) -> Result<Vec<JsonPerfMetrics>, HttpError> {
    let permutations = metric_kinds.len() * branches.len() * testbeds.len() * benchmarks.len();
    let gt_max_permutations = permutations > MAX_PERMUTATIONS;
    let mut results = Vec::with_capacity(permutations.min(MAX_PERMUTATIONS));
    for (metric_kind_index, metric_kind_uuid) in metric_kinds.iter().enumerate() {
        for (branch_index, branch_uuid) in branches.iter().enumerate() {
            for (testbed_index, testbed_uuid) in testbeds.iter().enumerate() {
                for (benchmark_index, benchmark_uuid) in benchmarks.iter().enumerate() {
                    if gt_max_permutations
                        && (metric_kind_index + 1)
                            * (branch_index + 1)
                            * (testbed_index + 1)
                            * (benchmark_index + 1)
                            > MAX_PERMUTATIONS
                    {
                        return Ok(results);
                    }

                    if let Some(perf_metrics) = perf_query(
                        conn,
                        project,
                        *metric_kind_uuid,
                        *branch_uuid,
                        *testbed_uuid,
                        *benchmark_uuid,
                        times,
                    )?
                    .into_iter()
                    .fold(
                        None,
                        |perf_metrics: Option<JsonPerfMetrics>, perf_query| {
                            let (query_dimensions, query) = split_perf_query(perf_query);
                            let perf_metric = new_perf_metric(project, query);
                            if let Some(mut perf_metrics) = perf_metrics {
                                perf_metrics.metrics.push(perf_metric);
                                Some(perf_metrics)
                            } else {
                                Some(new_perf_metrics(project, query_dimensions, perf_metric))
                            }
                        },
                    ) {
                        results.push(perf_metrics);
                    }
                }
            }
        }
    }
    Ok(results)
}

fn perf_query(
    conn: &mut DbConnection,
    project: &QueryProject,
    metric_kind_uuid: MetricKindUuid,
    branch_uuid: BranchUuid,
    testbed_uuid: TestbedUuid,
    benchmark_uuid: BenchmarkUuid,
    times: Times,
) -> Result<Vec<PerfQuery>, HttpError> {
    let mut query = schema::metric::table
        .inner_join(schema::metric_kind::table)
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
        .filter(schema::metric_kind::uuid.eq(metric_kind_uuid))
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
            QueryMetricKind::as_select(),
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
                    schema::boundary::baseline,
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
                ).nullable(),
            ).nullable(),
            QueryMetric::as_select(),
        ))
        .load::<PerfQuery>(conn)
        .map_err(resource_not_found_err!(Metric, (project, metric_kind_uuid, branch_uuid, testbed_uuid, benchmark_uuid)))
}

type PerfQuery = (
    QueryMetricKind,
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
    metric_kind: QueryMetricKind,
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

fn split_perf_query(
    (
        metric_kind,
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
        metric_kind,
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

fn new_perf_metrics(
    project: &QueryProject,
    query_dimensions: QueryDimensions,
    metric: JsonPerfMetric,
) -> JsonPerfMetrics {
    let QueryDimensions {
        metric_kind,
        branch,
        testbed,
        benchmark,
    } = query_dimensions;
    JsonPerfMetrics {
        metric_kind: metric_kind.into_json_for_project(project),
        branch: branch.into_json_for_project(project),
        testbed: testbed.into_json_for_project(project),
        benchmark: benchmark.into_json_for_project(project),
        metrics: vec![metric],
    }
}

fn new_perf_metric(
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
