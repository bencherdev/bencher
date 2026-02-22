use bencher_endpoint::{CorsResponse, Endpoint, Get, ResponseOk};
#[cfg(feature = "plus")]
use bencher_json::SpecUuid;
use bencher_json::{
    BenchmarkUuid, BranchUuid, DateTime, GitHash, HeadUuid, JsonPerf, JsonPerfQuery, MeasureUuid,
    ProjectResourceId, ReportUuid, TestbedUuid,
    project::{
        alert::JsonPerfAlert,
        head::{JsonVersion, VersionNumber},
        perf::{JsonPerfMetric, JsonPerfMetrics, JsonPerfQueryParams},
        report::Iteration,
        threshold::JsonThresholdModel,
    },
};
#[cfg(feature = "plus")]
use bencher_schema::model::spec::{QuerySpec, SpecId};
use bencher_schema::{
    context::{ApiContext, DbConnection},
    error::{bad_request_error, resource_not_found_err},
    model::{
        project::{
            QueryProject,
            benchmark::QueryBenchmark,
            branch::{QueryBranch, head::QueryHead},
            measure::QueryMeasure,
            metric_boundary::QueryMetricBoundary,
            testbed::QueryTestbed,
            threshold::{
                QueryThreshold, alert::QueryAlert, boundary::QueryBoundary, model::QueryModel,
            },
        },
        user::public::{PubBearerToken, PublicUser},
    },
    public_conn, schema, view,
};
use diesel::{
    ExpressionMethods as _, JoinOnDsl as _, NullableExpressionMethods as _, QueryDsl as _,
    RunQueryDsl as _, SelectableHelper as _,
};
use dropshot::{HttpError, Path, Query, RequestContext, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;

pub mod img;

const MAX_PERMUTATIONS: usize = 255;

#[derive(Deserialize, JsonSchema)]
pub struct ProjPerfParams {
    /// The slug or UUID for a project.
    pub project: ProjectResourceId,
}

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

    let public_user = PublicUser::from_token(
        &rqctx.log,
        rqctx.context(),
        #[cfg(feature = "plus")]
        rqctx.request.headers(),
        bearer_token,
    )
    .await?;
    let json = get_inner(
        rqctx.context(),
        path_params.into_inner(),
        json_perf_query,
        &public_user,
    )
    .await?;
    Ok(Get::response_ok(json, public_user.is_auth()))
}

async fn get_inner(
    context: &ApiContext,
    path_params: ProjPerfParams,
    json_perf_query: JsonPerfQuery,
    public_user: &PublicUser,
) -> Result<JsonPerf, HttpError> {
    let project = QueryProject::is_allowed_public(
        public_conn!(context, public_user),
        &context.rbac,
        &path_params.project,
        public_user,
    )?;

    let JsonPerfQuery {
        branches,
        heads,
        testbeds,
        #[cfg(feature = "plus")]
        specs,
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
        public_user,
        &project,
        &branches,
        &heads,
        &testbeds,
        #[cfg(feature = "plus")]
        &specs,
        &benchmarks,
        &measures,
        times,
    )
    .await?;

    Ok(JsonPerf {
        project: project.into_json(public_conn!(context, public_user))?,
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

#[expect(clippy::too_many_arguments)]
async fn perf_results(
    context: &ApiContext,
    public_user: &PublicUser,
    project: &QueryProject,
    branches: &[BranchUuid],
    heads: &[Option<HeadUuid>],
    testbeds: &[TestbedUuid],
    #[cfg(feature = "plus")] specs: &[Option<SpecUuid>],
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
            #[cfg(feature = "plus")]
            let spec_id = if let Some(spec_uuid) = specs.get(testbed_index).copied().flatten() {
                Some(QuerySpec::get_id(
                    public_conn!(context, public_user),
                    spec_uuid,
                )?)
            } else {
                None
            };

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
                        public_user,
                        project,
                        *branch_uuid,
                        *head_uuid,
                        *testbed_uuid,
                        #[cfg(feature = "plus")]
                        spec_id,
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
                                public_conn!(context, public_user),
                                project,
                                query_dimensions,
                                perf_metric,
                                #[cfg(feature = "plus")]
                                spec_id,
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

#[expect(clippy::too_many_arguments, clippy::too_many_lines)]
async fn perf_query(
    context: &ApiContext,
    public_user: &PublicUser,
    project: &QueryProject,
    branch_uuid: BranchUuid,
    head_uuid: Option<HeadUuid>,
    testbed_uuid: TestbedUuid,
    #[cfg(feature = "plus")] spec_id: Option<SpecId>,
    benchmark_uuid: BenchmarkUuid,
    measure_uuid: MeasureUuid,
    times: Times,
) -> Result<Vec<PerfQuery>, HttpError> {
    let mut query = view::metric_boundary::table
        .inner_join(
            schema::report_benchmark::table.inner_join(
                schema::report::table
                    .inner_join(schema::version::table
                        .inner_join(schema::head_version::table
                            .inner_join(schema::head::table
                                .on(schema::head_version::head_id.eq(schema::head::id)),
                            )
                            .inner_join(schema::branch::table.on(schema::head::branch_id.eq(schema::branch::id))),
                        ),
                    )
                    .inner_join(schema::testbed::table)
            )
            .inner_join(schema::benchmark::table)
        )
        .inner_join(schema::measure::table)
        // It is important to filter for the branch through the `head_version` table
        // and NOT on the head in the `report` table.
        // This is because the `head_version` table is the one that is updated
        // when a head is cloned/used as a start point.
        // In contrast, the `report` table is only set to a single head when the report is created.
        // Therefore, querying from the `report` table's `head` would not return results for any other heads.
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

    // Filter for the branch head if it is provided.
    // Otherwise, filter for the current, non-replaced head.
    if let Some(head_uuid) = head_uuid {
        query = query.filter(schema::head::uuid.eq(head_uuid));
    } else {
        query = query.filter(schema::branch::head_id.eq(schema::head::id.nullable()));
    }

    // Filter for the hardware spec if it is provided.
    // This filters reports that have a job linked to the given spec.
    // The subquery is executed separately to avoid Diesel trait solver limitations
    // with deeply nested join trees.
    #[cfg(feature = "plus")]
    if let Some(spec_id) = spec_id {
        let report_ids: Vec<bencher_schema::model::project::report::ReportId> =
            schema::job::table
                .filter(schema::job::spec_id.eq(spec_id))
                .select(schema::job::report_id)
                .load(public_conn!(context, public_user))
                .map_err(resource_not_found_err!(Metric, spec_id))?;
        query = query.filter(schema::report::id.eq_any(report_ids));
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

    let query = query
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
            QueryHead::as_select(),
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
        ));

    // Use this to print the raw SQL query
    // https://bencher.dev/learn/engineering/sqlite-performance-tuning/
    // println!("{}", diesel::debug_query(&query).to_string());

    query
        // Acquire the lock on the database connection for every query.
        // This helps to avoid resource contention when the database is under heavy load.
        // This will make the perf query itself slower, but it will make the overall system more stable.
        .load::<PerfQuery>(public_conn!(context, public_user))
        .map_err(resource_not_found_err!(Metric, (project,  branch_uuid, testbed_uuid, benchmark_uuid, measure_uuid)))
}

type PerfQuery = (
    QueryBranch,
    QueryHead,
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
    head: QueryHead,
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
            Some(query_threshold.into_threshold_model_json_for_project(project, query_model));
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
    #[cfg(feature = "plus")] spec_id: Option<SpecId>,
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
        testbed: testbed.into_json_for_spec(
            conn,
            project,
            #[cfg(feature = "plus")]
            spec_id,
        )?,
        benchmark: benchmark.into_json_for_project(project),
        measure: measure.into_json_for_project(project),
        metrics: vec![metric],
    })
}
