use bencher_json::{JsonDirection, JsonMetricKind, JsonNewMetricKind, JsonPagination, ResourceId};
use bencher_rbac::project::Permission;
use diesel::{expression_methods::BoolExpressionMethods, ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::{endpoint, HttpError, Path, Query, RequestContext, TypedBody};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    context::ApiContext,
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
pub struct ProjMetricKindsParams {
    pub project: ResourceId,
}

pub type ProjMetricKindsQuery = JsonPagination<ProjMetricKindsSort, ProjMetricKindsQueryParams>;

#[derive(Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProjMetricKindsSort {
    #[default]
    Name,
}

#[derive(Deserialize, JsonSchema)]
pub struct ProjMetricKindsQueryParams {
    pub name: Option<String>,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/metric-kinds",
    tags = ["projects", "metric kinds"]
}]
pub async fn proj_metric_kinds_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjMetricKindsParams>,
    _query_params: Query<ProjMetricKindsQuery>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<ApiContext>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/metric-kinds",
    tags = ["projects", "metric kinds"]
}]
pub async fn proj_metric_kinds_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjMetricKindsParams>,
    query_params: Query<ProjMetricKindsQuery>,
) -> Result<ResponseOk<Vec<JsonMetricKind>>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await.ok();
    let endpoint = Endpoint::new(METRIC_KIND_RESOURCE, Method::GetLs);

    let json = get_ls_inner(
        rqctx.context(),
        auth_user.as_ref(),
        path_params.into_inner(),
        query_params.into_inner(),
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
    context: &ApiContext,
    auth_user: Option<&AuthUser>,
    path_params: ProjMetricKindsParams,
    query_params: ProjMetricKindsQuery,
    endpoint: Endpoint,
) -> Result<Vec<JsonMetricKind>, ApiError> {
    let conn = &mut *context.conn().await;

    let query_project =
        QueryProject::is_allowed_public(conn, &context.rbac, &path_params.project, auth_user)?;

    let mut query = schema::metric_kind::table
        .filter(schema::metric_kind::project_id.eq(&query_project.id))
        .into_boxed();

    if let Some(name) = &query_params.query.name {
        query = query.filter(schema::metric_kind::name.eq(name));
    }

    query = match query_params.order() {
        ProjMetricKindsSort::Name => match query_params.direction {
            Some(JsonDirection::Asc) | None => query.order(schema::metric_kind::name.asc()),
            Some(JsonDirection::Desc) => query.order(schema::metric_kind::name.desc()),
        },
    };

    Ok(query
        .offset(query_params.offset())
        .limit(query_params.limit())
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
pub async fn proj_metric_kind_post(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjMetricKindsParams>,
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
    context: &ApiContext,
    path_params: ProjMetricKindsParams,
    json_metric_kind: JsonNewMetricKind,
    auth_user: &AuthUser,
) -> Result<JsonMetricKind, ApiError> {
    let conn = &mut *context.conn().await;

    let insert_metric_kind =
        InsertMetricKind::from_json(conn, &path_params.project, json_metric_kind)?;
    // Verify that the user is allowed
    QueryProject::is_allowed_id(
        conn,
        &context.rbac,
        insert_metric_kind.project_id,
        auth_user,
        Permission::Create,
    )?;

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
pub struct ProjMetricKindParams {
    pub project: ResourceId,
    pub metric_kind: ResourceId,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/metric-kinds/{metric_kind}",
    tags = ["projects", "metric kinds"]
}]
pub async fn proj_metric_kind_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjMetricKindParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<ApiContext>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/metric-kinds/{metric_kind}",
    tags = ["projects", "metric kinds"]
}]
pub async fn proj_metric_kind_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjMetricKindParams>,
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
    context: &ApiContext,
    path_params: ProjMetricKindParams,
    auth_user: Option<&AuthUser>,
) -> Result<JsonMetricKind, ApiError> {
    let conn = &mut *context.conn().await;

    let query_project =
        QueryProject::is_allowed_public(conn, &context.rbac, &path_params.project, auth_user)?;

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
