use std::sync::Arc;

use bencher_json::{JsonMetricKind, JsonNewMetricKind, ResourceId};
use bencher_rbac::project::Permission;
use diesel::{expression_methods::BoolExpressionMethods, ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::{endpoint, HttpError, Path, RequestContext, TypedBody};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    context::Context,
    endpoints::{
        endpoint::{pub_response_ok, response_accepted, response_ok, ResponseAccepted, ResponseOk},
        Endpoint, Method,
    },
    error::api_error,
    model::project::{
        metric_kind::{InsertMetricKind, QueryMetricKind},
        QueryProject,
    },
    model::user::auth::AuthUser,
    schema,
    util::{
        cors::{get_cors, CorsResponse},
        error::into_json,
        resource_id::fn_resource_id,
    },
    ApiError,
};

use super::Resource;

const METRIC_KIND_RESOURCE: Resource = Resource::MetricKind;

#[derive(Deserialize, JsonSchema)]
pub struct DirPath {
    pub project: ResourceId,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/metric-kinds",
    tags = ["projects", "metric kinds"]
}]
pub async fn dir_options(
    _rqctx: Arc<RequestContext<Context>>,
    _path_params: Path<DirPath>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/metric-kinds",
    tags = ["projects", "metric kinds"]
}]
pub async fn get_ls(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<DirPath>,
) -> Result<ResponseOk<Vec<JsonMetricKind>>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await.ok();
    let endpoint = Endpoint::new(METRIC_KIND_RESOURCE, Method::GetLs);

    let json = get_ls_inner(
        rqctx.context(),
        auth_user.as_ref(),
        path_params.into_inner(),
        endpoint,
    )
    .await
    .map_err(|e| endpoint.err(e))?;

    if auth_user.is_some() {
        response_ok!(endpoint, json)
    } else {
        pub_response_ok!(endpoint, json)
    }
}

async fn get_ls_inner(
    context: &Context,
    auth_user: Option<&AuthUser>,
    path_params: DirPath,
    endpoint: Endpoint,
) -> Result<Vec<JsonMetricKind>, ApiError> {
    let api_context = &mut *context.lock().await;
    let query_project =
        QueryProject::is_allowed_public(api_context, &path_params.project, auth_user)?;
    let conn = &mut api_context.database.connection;

    Ok(schema::metric_kind::table
        .filter(schema::metric_kind::project_id.eq(query_project.id))
        .order((schema::metric_kind::name, schema::metric_kind::slug))
        .load::<QueryMetricKind>(conn)
        .map_err(api_error!())?
        .into_iter()
        .filter_map(into_json!(endpoint, conn))
        .collect())
}

#[endpoint {
    method = POST,
    path =  "/v0/projects/{project}/metric-kinds",
    tags = ["projects", "metric kinds"]
}]
pub async fn post(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<DirPath>,
    body: TypedBody<JsonNewMetricKind>,
) -> Result<ResponseAccepted<JsonMetricKind>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(METRIC_KIND_RESOURCE, Method::Post);

    let json = post_inner(
        rqctx.context(),
        path_params.into_inner(),
        body.into_inner(),
        &auth_user,
    )
    .await
    .map_err(|e| endpoint.err(e))?;

    response_accepted!(endpoint, json)
}

async fn post_inner(
    context: &Context,
    path_params: DirPath,
    json_metric_kind: JsonNewMetricKind,
    auth_user: &AuthUser,
) -> Result<JsonMetricKind, ApiError> {
    let api_context = &mut *context.lock().await;
    let insert_metric_kind = InsertMetricKind::from_json(
        &mut api_context.database.connection,
        &path_params.project,
        json_metric_kind,
    )?;
    // Verify that the user is allowed
    QueryProject::is_allowed_id(
        api_context,
        insert_metric_kind.project_id,
        auth_user,
        Permission::Create,
    )?;
    let conn = &mut api_context.database.connection;

    diesel::insert_into(schema::metric_kind::table)
        .values(&insert_metric_kind)
        .execute(conn)
        .map_err(api_error!())?;

    schema::metric_kind::table
        .filter(schema::metric_kind::uuid.eq(&insert_metric_kind.uuid))
        .first::<QueryMetricKind>(conn)
        .map_err(api_error!())?
        .into_json(conn)
}

#[derive(Deserialize, JsonSchema)]
pub struct OnePath {
    pub project: ResourceId,
    pub metric_kind: ResourceId,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/metric-kinds/{metric_kind}",
    tags = ["projects", "metric kinds"]
}]
pub async fn one_options(
    _rqctx: Arc<RequestContext<Context>>,
    _path_params: Path<OnePath>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/metric-kinds/{metric_kind}",
    tags = ["projects", "metric kinds"]
}]
pub async fn get_one(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<OnePath>,
) -> Result<ResponseOk<JsonMetricKind>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await.ok();
    let endpoint = Endpoint::new(METRIC_KIND_RESOURCE, Method::GetOne);

    let json = get_one_inner(
        rqctx.context(),
        path_params.into_inner(),
        auth_user.as_ref(),
    )
    .await
    .map_err(|e| endpoint.err(e))?;

    if auth_user.is_some() {
        response_ok!(endpoint, json)
    } else {
        pub_response_ok!(endpoint, json)
    }
}

fn_resource_id!(metric_kind);

async fn get_one_inner(
    context: &Context,
    path_params: OnePath,
    auth_user: Option<&AuthUser>,
) -> Result<JsonMetricKind, ApiError> {
    let api_context = &mut *context.lock().await;
    let query_project =
        QueryProject::is_allowed_public(api_context, &path_params.project, auth_user)?;
    let conn = &mut api_context.database.connection;

    schema::metric_kind::table
        .filter(
            schema::metric_kind::project_id
                .eq(query_project.id)
                .and(resource_id(&path_params.metric_kind)?),
        )
        .first::<QueryMetricKind>(conn)
        .map_err(api_error!())?
        .into_json(conn)
}
