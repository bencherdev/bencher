use bencher_json::{
    project::{
        alert::JsonPerfAlert,
        perf::{JsonPerfMetric, JsonPerfMetrics, JsonPerfQueryParams},
        reference::{JsonVersion, VersionNumber},
        report::Iteration,
        threshold::JsonThresholdModel,
    },
    BenchmarkUuid, BranchUuid, DateTime, GitHash, JsonPerf, JsonPerfQuery, MeasureUuid,
    ReferenceUuid, ReportUuid, ResourceId, TestbedUuid,
};
use diesel::{
    ExpressionMethods, JoinOnDsl, NullableExpressionMethods, QueryDsl, RunQueryDsl,
    SelectableHelper,
};
use dropshot::{endpoint, HttpError, Path, Query, RequestContext};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    conn_lock,
    context::{ApiContext, DbConnection},
    endpoints::{
        endpoint::{CorsResponse, Get, ResponseOk},
        Endpoint,
    },
    error::{bad_request_error, resource_not_found_err},
    model::{
        project::{
            benchmark::QueryBenchmark,
            branch::{reference::QueryReference, QueryBranch},
            measure::QueryMeasure,
            metric_boundary::QueryMetricBoundary,
            testbed::QueryTestbed,
            threshold::{
                alert::QueryAlert, boundary::QueryBoundary, model::QueryModel, QueryThreshold,
            },
            QueryProject,
        },
        user::auth::{AuthUser, PubBearerToken},
    },
    schema, view,
};

pub mod img;

const MAX_PERMUTATIONS: usize = 255;

#[derive(Deserialize, JsonSchema)]
pub struct ProjPerfParams {
    /// The slug or UUID for a project.
    pub project: ResourceId,
}

#[allow(clippy::no_effect_underscore_binding, clippy::unused_async)]
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

/// Query project performance metrics
///
/// Query the performance metrics for a project.
/// The query results are every permutation of each branch, testbed, benchmark, and measure.
/// There is a limit of 255 permutations for a single request.
/// Therefore, only the first 255 permutations are returned.
/// If the project is public, then the user does not need to be authenticated.
/// If the project is private, then the user must be authenticated and have `view` permissions for the project.
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
    let project = QueryProject::is_allowed_public(
        conn_lock!(context),
        &context.rbac,
        &path_params.project,
        auth_user,
    )?;

    let JsonPerfQuery {
        branches,
        heads,
        testbeds,
        benchmarks,
        measures,
        start_time,
        end_time,
    } = json_perf_query;

    let times = Times {
        start_time,
        end_time,
    };

    let results = perf_results(
        context,
        &project,
        &branches,
        &heads,
        &testbeds,
        &benchmarks,
        &measures,
        times,
    )
    .await?;

    Ok(JsonPerf {
        project: project.into_json(conn_lock!(context))?,
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

#[allow(clippy::too_many_arguments)]
async fn perf_results(
    context: &ApiContext,
    project: &QueryProject,
    branches: &[BranchUuid],
    heads: &[Option<ReferenceUuid>],
    testbeds: &[TestbedUuid],
    benchmarks: &[BenchmarkUuid],
    measures: &[MeasureUuid],
    times: Times,
) -> Result<Vec<JsonPerfMetrics>, HttpError> {
    let permutations = branches.len() * testbeds.len() * benchmarks.len() * measures.len();
    let gt_max_permutations = permutations > MAX_PERMUTATIONS;
    let mut results = Vec::with_capacity(permutations.min(MAX_PERMUTATIONS));
    // It is okay to use `zip` because `JsonPerfQuery` guarantees that the lengths are the same.
    for (branch_index, (branch_uuid, head_uuid)) in branches.iter().zip(heads.iter()).enumerate() {
        for (testbed_index, testbed_uuid) in testbeds.iter().enumerate() {
            for (benchmark_index, benchmark_uuid) in benchmarks.iter().enumerate() {
                for (measure_index, measure_uuid) in measures.iter().enumerate() {
                    if gt_max_permutations
                        && (branch_index + 1)
                            * (testbed_index + 1)
                            * (benchmark_index + 1)
                            * (measure_index + 1)
                            > MAX_PERMUTATIONS
                    {
                        return Ok(results);
                    }

                    let pq = perf_query(
                        context,
                        project,
                        *branch_uuid,
                        *head_uuid,
                        *testbed_uuid,
                        *benchmark_uuid,
                        *measure_uuid,
                        times,
                    )
                    .await?;

                    let mut perf_metrics: Option<JsonPerfMetrics> = None;
                    for (query_dimensions, perf_metric) in
                        pq.into_iter().map(|pq| split_perf_query(project, pq))
                    {
                        if let Some(perf_metrics) = &mut perf_metrics {
                            perf_metrics.metrics.push(perf_metric);
                        } else {
                            perf_metrics = new_perf_metrics(
                                conn_lock!(context),
                                project,
                                query_dimensions,
                                perf_metric,
                            )
                            .ok();
                        }
                    }
                    if let Some(perf_metrics) = perf_metrics.take() {
                        results.push(perf_metrics);
                    }
                }
            }
        }
    }
    Ok(results)
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
async fn perf_query(
    context: &ApiContext,
    project: &QueryProject,
    branch_uuid: BranchUuid,
    head_uuid: Option<ReferenceUuid>,
    testbed_uuid: TestbedUuid,
    benchmark_uuid: BenchmarkUuid,
    measure_uuid: MeasureUuid,
    times: Times,
) -> Result<Vec<PerfQuery>, HttpError> {
    let mut query =
        view::metric_boundary::table
        .inner_join(
            schema::report_benchmark::table.inner_join(
                schema::report::table
                    .inner_join(schema::version::table
                        .inner_join(schema::reference_version::table
                            .inner_join(schema::reference::table
                                .on(schema::reference_version::reference_id.eq(schema::reference::id)),
                            )
                            .inner_join(schema::branch::table.on(schema::reference::branch_id.eq(schema::branch::id))),
                        ),
                    )
                    .inner_join(schema::testbed::table)
            )
            .inner_join(schema::benchmark::table)
        )
        .inner_join(schema::measure::table)
        // It is important to filter for the branch through the `reference_version` table
        // and NOT on the reference in the `report` table.
        // This is because the `reference_version` table is the one that is updated
        // when a reference is cloned/used as a start point.
        // In contrast, the `report` table is only set to a single reference when the report is created.
        // Therefore, querying from the `report` table's `reference` would not return results for any other references.
        .filter(schema::branch::uuid.eq(branch_uuid))
        .filter(schema::testbed::uuid.eq(testbed_uuid))
        .filter(schema::benchmark::uuid.eq(benchmark_uuid))
        .filter(schema::measure::uuid.eq(measure_uuid))
        // Make sure that the project is the same for all dimensions
        .filter(schema::branch::project_id.eq(project.id))
        .filter(schema::testbed::project_id.eq(project.id))
        .filter(schema::benchmark::project_id.eq(project.id))
        .filter(schema::measure::project_id.eq(project.id))
        // There may or may not be a boundary for any given metric
        .left_join(schema::threshold::table)
        .left_join(schema::model::table)
        // There may or may not be an alert for any given boundary
        .left_join(schema::alert::table.on(view::metric_boundary::boundary_id.eq(schema::alert::boundary_id.nullable())))
        .into_boxed();

    // Filter for the branch head reference if it is provided.
    // Otherwise, filter for the current, non-replaced head reference.
    if let Some(reference_uuid) = head_uuid {
        query = query.filter(schema::reference::uuid.eq(reference_uuid));
    } else {
        query = query.filter(schema::reference::replaced.is_null());
    }

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
            schema::report_benchmark::iteration,
        ))
        .select((
            QueryBranch::as_select(),
            QueryReference::as_select(),
            QueryTestbed::as_select(),
            QueryBenchmark::as_select(),
            QueryMeasure::as_select(),
            schema::report::uuid,
            schema::report_benchmark::iteration,
            schema::report::start_time,
            schema::report::end_time,
            schema::version::number,
            schema::version::hash,
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
            QueryMetricBoundary::as_select(),
        ))
        // Acquire the lock on the database connection for every query.
        // This helps to avoid resource contention when the database is under heavy load.
        // This will make the perf query itself slower, but it will make the overall system more stable.
        .load::<PerfQuery>(conn_lock!(context))
        .map_err(resource_not_found_err!(Metric, (project,  branch_uuid, testbed_uuid, benchmark_uuid, measure_uuid)))
}

type PerfQuery = (
    QueryBranch,
    QueryReference,
    QueryTestbed,
    QueryBenchmark,
    QueryMeasure,
    ReportUuid,
    Iteration,
    DateTime,
    DateTime,
    VersionNumber,
    Option<GitHash>,
    Option<(QueryThreshold, QueryModel, Option<QueryAlert>)>,
    QueryMetricBoundary,
);

struct QueryDimensions {
    branch: QueryBranch,
    head: QueryReference,
    testbed: QueryTestbed,
    benchmark: QueryBenchmark,
    measure: QueryMeasure,
}

type PerfMetricQuery = (
    ReportUuid,
    Iteration,
    DateTime,
    DateTime,
    VersionNumber,
    Option<GitHash>,
    Option<(QueryThreshold, QueryModel, Option<QueryAlert>)>,
    QueryMetricBoundary,
);

fn split_perf_query(
    project: &QueryProject,
    (
        branch,
        head,
        testbed,
        benchmark,
        measure,
        report_uuid,
        iteration,
        start_time,
        end_time,
        version_number,
        version_hash,
        boundary_limit,
        query_metric_boundary,
    ): PerfQuery,
) -> (QueryDimensions, JsonPerfMetric) {
    let query_dimensions = QueryDimensions {
        branch,
        head,
        testbed,
        benchmark,
        measure,
    };
    let metric_query = (
        report_uuid,
        iteration,
        start_time,
        end_time,
        version_number,
        version_hash,
        boundary_limit,
        query_metric_boundary,
    );
    (query_dimensions, new_perf_metric(project, metric_query))
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
        tma,
        query_metric_boundary,
    ): PerfMetricQuery,
) -> JsonPerfMetric {
    let version = JsonVersion {
        number: version_number,
        hash: version_hash,
    };

    let (threshold, alert) = threshold_model_alert(project, tma);
    let (metric, boundary) = QueryMetricBoundary::split(query_metric_boundary);
    let metric = metric.into_json();
    let boundary = boundary.map(QueryBoundary::into_json);

    JsonPerfMetric {
        report: report_uuid,
        iteration,
        start_time,
        end_time,
        version,
        metric,
        threshold,
        boundary,
        alert,
    }
}

pub(super) fn threshold_model_alert(
    project: &QueryProject,
    tma: Option<(QueryThreshold, QueryModel, Option<QueryAlert>)>,
) -> (Option<JsonThresholdModel>, Option<JsonPerfAlert>) {
    if let Some((query_threshold, query_model, query_alert)) = tma {
        let threshold =
            Some(query_threshold.into_threshold_model_json_for_project(project, Some(query_model)));
        let alert = query_alert.map(QueryAlert::into_perf_json);
        (threshold, alert)
    } else {
        (None, None)
    }
}

fn new_perf_metrics(
    conn: &mut DbConnection,
    project: &QueryProject,
    query_dimensions: QueryDimensions,
    metric: JsonPerfMetric,
) -> Result<JsonPerfMetrics, HttpError> {
    let QueryDimensions {
        branch,
        head,
        testbed,
        benchmark,
        measure,
    } = query_dimensions;
    Ok(JsonPerfMetrics {
        branch: branch.into_json_for_head(conn, project, &head, None)?,
        testbed: testbed.into_json_for_project(project),
        benchmark: benchmark.into_json_for_project(project),
        measure: measure.into_json_for_project(project),
        metrics: vec![metric],
    })
}
