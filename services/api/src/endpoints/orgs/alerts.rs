use std::sync::Arc;

use bencher_json::{JsonAlert, ResourceId};
use bencher_rbac::{
    organization::Permission as OrganizationPermission, project::Permission as ProjectPermission,
};
use diesel::{ExpressionMethods, JoinOnDsl, QueryDsl, RunQueryDsl};
use dropshot::{endpoint, HttpError, Path, RequestContext};
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    endpoints::{
        endpoint::{response_ok, ResponseOk},
        Endpoint, Method,
    },
    error::api_error,
    model::{project::QueryProject, threshold::alert::QueryAlert, user::auth::AuthUser},
    schema,
    util::{
        cors::{get_cors, CorsResponse},
        Context,
    },
    ApiError,
};

use super::Resource;

const ALERT_RESOURCE: Resource = Resource::Alert;

#[derive(Deserialize, JsonSchema)]
pub struct GetLsParams {
    pub project: ResourceId,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/alerts",
    tags = ["projects", "alerts"]
}]
pub async fn dir_options(
    _rqctx: Arc<RequestContext<Context>>,
    _path_params: Path<GetLsParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/alerts",
    tags = ["projects", "alerts"]
}]
pub async fn get_ls(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<GetLsParams>,
) -> Result<ResponseOk<Vec<JsonAlert>>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(ALERT_RESOURCE, Method::GetLs);

    let json = get_ls_inner(rqctx.context(), &auth_user, path_params.into_inner())
        .await
        .map_err(|e| endpoint.err(e))?;

    response_ok!(endpoint, json)
}

async fn get_ls_inner(
    context: &Context,
    auth_user: &AuthUser,
    path_params: GetLsParams,
) -> Result<Vec<JsonAlert>, ApiError> {
    let api_context = &mut *context.lock().await;
    let query_project = QueryProject::is_allowed_resource_id(
        api_context,
        &path_params.project,
        auth_user,
        OrganizationPermission::Manage,
        ProjectPermission::View,
    )?;
    let conn = &mut api_context.db_conn;

    Ok(schema::alert::table
        .left_join(schema::perf::table.on(schema::alert::perf_id.eq(schema::perf::id)))
        .left_join(
            schema::benchmark::table.on(schema::perf::benchmark_id.eq(schema::benchmark::id)),
        )
        .filter(schema::benchmark::project_id.eq(query_project.id))
        .left_join(schema::report::table.on(schema::perf::report_id.eq(schema::report::id)))
        .order((schema::report::start_time, schema::perf::iteration))
        .select((
            schema::alert::id,
            schema::alert::uuid,
            schema::alert::perf_id,
            schema::alert::threshold_id,
            schema::alert::statistic_id,
            schema::alert::side,
            schema::alert::boundary,
            schema::alert::outlier,
        ))
        .load::<QueryAlert>(conn)
        .map_err(api_error!())?
        .into_iter()
        .filter_map(|query| query.into_json(conn).ok())
        .collect())
}

#[derive(Deserialize, JsonSchema)]
pub struct GetOneParams {
    pub project: ResourceId,
    pub alert: Uuid,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/alerts/{alert}",
    tags = ["projects", "alerts"]
}]
pub async fn one_options(
    _rqctx: Arc<RequestContext<Context>>,
    _path_params: Path<GetOneParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/alerts/{alert}",
    tags = ["projects", "alerts"]
}]
pub async fn get_one(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<GetOneParams>,
) -> Result<ResponseOk<JsonAlert>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(ALERT_RESOURCE, Method::GetOne);

    let json = get_one_inner(rqctx.context(), path_params.into_inner(), &auth_user)
        .await
        .map_err(|e| endpoint.err(e))?;

    response_ok!(endpoint, json)
}

async fn get_one_inner(
    context: &Context,
    path_params: GetOneParams,
    auth_user: &AuthUser,
) -> Result<JsonAlert, ApiError> {
    let api_context = &mut *context.lock().await;
    let query_project = QueryProject::is_allowed_resource_id(
        api_context,
        &path_params.project,
        auth_user,
        OrganizationPermission::Manage,
        ProjectPermission::View,
    )?;
    let conn = &mut api_context.db_conn;

    schema::alert::table
        .left_join(schema::perf::table.on(schema::alert::perf_id.eq(schema::perf::id)))
        .left_join(
            schema::benchmark::table.on(schema::perf::benchmark_id.eq(schema::benchmark::id)),
        )
        .filter(schema::benchmark::project_id.eq(query_project.id))
        .filter(schema::alert::uuid.eq(path_params.alert.to_string()))
        .select((
            schema::alert::id,
            schema::alert::uuid,
            schema::alert::perf_id,
            schema::alert::threshold_id,
            schema::alert::statistic_id,
            schema::alert::side,
            schema::alert::boundary,
            schema::alert::outlier,
        ))
        .first::<QueryAlert>(conn)
        .map_err(api_error!())?
        .into_json(conn)
}
