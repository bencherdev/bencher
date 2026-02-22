use bencher_endpoint::{CorsResponse, Endpoint, Get, ResponseOk};
use bencher_json::{
    DateTime, JsonOneMetric, MetricUuid, ProjectResourceId, ReportUuid, project::report::Iteration,
};
#[cfg(feature = "plus")]
use bencher_schema::model::runner::job::QueryJob;
use bencher_schema::{
    context::{ApiContext, DbConnection},
    error::resource_not_found_err,
    model::{
        project::{
            QueryProject,
            benchmark::QueryBenchmark,
            branch::{QueryBranch, head::QueryHead, version::QueryVersion},
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
use dropshot::{HttpError, Path, RequestContext, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;

use super::perf::threshold_model_alert;

#[derive(Deserialize, JsonSchema)]
pub struct ProjMetricParams {
    /// The slug or UUID for a project.
    pub project: ProjectResourceId,
    /// The UUID for a metric.
    pub metric: MetricUuid,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/metrics/{metric}",
    tags = ["projects", "metrics"]
}]
pub async fn proj_metric_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjMetricParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

/// View a metric
///
/// View a metric for a project.
/// If the project is public, then the user does not need to be authenticated.
/// If the project is private, then the user must be authenticated and have `view` permissions for the project.
#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/metrics/{metric}",
    tags = ["projects", "metrics"]
}]
pub async fn proj_metric_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubBearerToken,
    path_params: Path<ProjMetricParams>,
) -> Result<ResponseOk<JsonOneMetric>, HttpError> {
    let public_user = PublicUser::from_token(
        &rqctx.log,
        rqctx.context(),
        #[cfg(feature = "plus")]
        rqctx.request.headers(),
        bearer_token,
    )
    .await?;
    let json = get_one_inner(rqctx.context(), path_params.into_inner(), &public_user).await?;
    Ok(Get::response_ok(json, public_user.is_auth()))
}

async fn get_one_inner(
    context: &ApiContext,
    path_params: ProjMetricParams,
    public_user: &PublicUser,
) -> Result<JsonOneMetric, HttpError> {
    let query_project = QueryProject::is_allowed_public(
        public_conn!(context, public_user),
        &context.rbac,
        &path_params.project,
        public_user,
    )?;

    public_conn!(context, public_user, |conn| {
        view::metric_boundary::table
        .inner_join(
            schema::report_benchmark::table.inner_join(
                schema::report::table
                    .inner_join(schema::version::table
                        .inner_join(schema::head_version::table
                            .inner_join(schema::head::table
                                .on(schema::head::id.eq(schema::head_version::head_id)),
                            ).inner_join(schema::branch::table.on(schema::branch::id.eq(schema::head::branch_id))),
                        ),
                    )
                    .inner_join(schema::testbed::table)
            )
            .inner_join(schema::benchmark::table)
        )
        .inner_join(schema::measure::table)
        .filter(view::metric_boundary::metric_uuid.eq(path_params.metric))
        // Make sure that the project is the same for all dimensions
        .filter(schema::branch::project_id.eq(query_project.id))
        .filter(schema::testbed::project_id.eq(query_project.id))
        .filter(schema::benchmark::project_id.eq(query_project.id))
        .filter(schema::measure::project_id.eq(query_project.id))
        // There may or may not be a boundary for any given metric
        .left_join(schema::threshold::table)
        .left_join(schema::model::table)
        // There may or may not be an alert for any given boundary
        .left_join(schema::alert::table.on(view::metric_boundary::boundary_id.eq(schema::alert::boundary_id.nullable())))
        .select((
            QueryBranch::as_select(),
            QueryHead::as_select(),
            QueryVersion::as_select(),
            QueryTestbed::as_select(),
            QueryBenchmark::as_select(),
            QueryMeasure::as_select(),
            schema::report::uuid,
            schema::report_benchmark::iteration,
            schema::report::start_time,
            schema::report::end_time,
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
        .first::<MetricQuery>(conn)
        .map_err(resource_not_found_err!(Metric, (&query_project,  &path_params.metric)))
        .map(|perf_query| metric_query_json(conn, &query_project, perf_query))?
    })
}

pub(super) type MetricQuery = (
    QueryBranch,
    QueryHead,
    QueryVersion,
    QueryTestbed,
    QueryBenchmark,
    QueryMeasure,
    ReportUuid,
    Iteration,
    DateTime,
    DateTime,
    Option<(QueryThreshold, QueryModel, Option<QueryAlert>)>,
    QueryMetricBoundary,
);

fn metric_query_json(
    conn: &mut DbConnection,
    project: &QueryProject,
    (
        branch,
        head,
        version,
        testbed,
        benchmark,
        measure,
        report,
        iteration,
        start_time,
        end_time,
        tma,
        query_metric_boundary,
    ): MetricQuery,
) -> Result<JsonOneMetric, HttpError> {
    #[cfg(feature = "plus")]
    let spec_id = QueryJob::spec_id_for_report_uuid(conn, &report)?;

    let branch = branch.into_json_for_head(conn, project, &head, Some(version))?;
    let testbed = testbed.into_json_for_spec(
        conn,
        project,
        #[cfg(feature = "plus")]
        spec_id,
    )?;
    let benchmark = benchmark.into_json_for_project(project);
    let measure = measure.into_json_for_project(project);

    let (threshold, alert) = threshold_model_alert(project, tma);
    let (metric, boundary) = QueryMetricBoundary::split(query_metric_boundary);
    let metric_uuid = metric.uuid;
    let metric = metric.into_json();
    let boundary = boundary.map(QueryBoundary::into_json);

    Ok(JsonOneMetric {
        uuid: metric_uuid,
        report,
        iteration,
        start_time,
        end_time,
        branch,
        testbed,
        benchmark,
        measure,
        metric,
        threshold,
        boundary,
        alert,
    })
}
