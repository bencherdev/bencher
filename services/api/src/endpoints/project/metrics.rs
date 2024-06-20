use bencher_json::{
    project::{branch::VersionNumber, report::Iteration},
    DateTime, GitHash, JsonIsolatedMetric, MetricUuid, ReportUuid, ResourceId,
};
use diesel::{
    ExpressionMethods, JoinOnDsl, NullableExpressionMethods, QueryDsl, RunQueryDsl,
    SelectableHelper,
};
use dropshot::{endpoint, HttpError, Path, RequestContext};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    conn_lock,
    context::ApiContext,
    endpoints::{
        endpoint::{CorsResponse, Get, ResponseOk},
        Endpoint,
    },
    error::resource_not_found_err,
    model::{
        project::{
            benchmark::QueryBenchmark,
            branch::QueryBranch,
            measure::QueryMeasure,
            metric_boundary::QueryMetricBoundary,
            testbed::QueryTestbed,
            threshold::{alert::QueryAlert, model::QueryModel, QueryThreshold},
            QueryProject,
        },
        user::auth::{AuthUser, PubBearerToken},
    },
    schema, view,
};

#[derive(Deserialize, JsonSchema)]
pub struct ProjMetricParams {
    /// The slug or UUID for a project.
    pub project: ResourceId,
    /// The UUID for a metric.
    pub metric: MetricUuid,
}

#[allow(clippy::no_effect_underscore_binding, clippy::unused_async)]
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
) -> Result<ResponseOk<JsonIsolatedMetric>, HttpError> {
    let auth_user = AuthUser::from_pub_token(rqctx.context(), bearer_token).await?;
    let json = get_one_inner(
        rqctx.context(),
        path_params.into_inner(),
        auth_user.as_ref(),
    )
    .await?;
    Ok(Get::response_ok(json, auth_user.is_some()))
}

async fn get_one_inner(
    context: &ApiContext,
    path_params: ProjMetricParams,
    auth_user: Option<&AuthUser>,
) -> Result<JsonIsolatedMetric, HttpError> {
    let query_project = QueryProject::is_allowed_public(
        conn_lock!(context),
        &context.rbac,
        &path_params.project,
        auth_user,
    )?;

    let query = view::metric_boundary::table
        .inner_join(
            schema::report_benchmark::table.inner_join(
                schema::report::table
                    .inner_join(schema::version::table
                        .inner_join(schema::branch_version::table
                            .inner_join(schema::branch::table
                                .on(schema::branch_version::branch_id.eq(schema::branch::id)),
                            )
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
        .first::<MetricQuery>(conn_lock!(context))
        .map_err(resource_not_found_err!(Metric, (query_project,  &path_params.metric)))?;

    todo!()
}

type MetricQuery = (
    QueryBranch,
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
