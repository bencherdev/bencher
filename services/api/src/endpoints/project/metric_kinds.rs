use bencher_json::{
    project::metric_kind::JsonUpdateMetricKind, JsonDirection, JsonEmpty, JsonMetricKind,
    JsonMetricKinds, JsonNewMetricKind, JsonPagination, NonEmpty, ResourceId,
};
use bencher_rbac::project::Permission;
use diesel::{BelongingToDsl, ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::{endpoint, HttpError, Path, Query, RequestContext, TypedBody};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    context::ApiContext,
    endpoints::{
        endpoint::{CorsResponse, Delete, Get, Patch, Post, ResponseAccepted, ResponseOk},
        Endpoint,
    },
    error::{resource_conflict_err, resource_not_found_err},
    model::user::auth::{AuthUser, PubBearerToken},
    model::{
        project::{
            metric_kind::{InsertMetricKind, QueryMetricKind, UpdateMetricKind},
            QueryProject,
        },
        user::auth::BearerToken,
    },
    schema,
};

#[derive(Deserialize, JsonSchema)]
pub struct ProjMetricKindsParams {
    pub project: ResourceId,
}

pub type ProjMetricKindsPagination = JsonPagination<ProjMetricKindsSort>;

#[derive(Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProjMetricKindsSort {
    #[default]
    Name,
}

#[derive(Deserialize, JsonSchema)]
pub struct ProjMetricKindsQuery {
    pub name: Option<NonEmpty>,
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
    _pagination_params: Query<ProjMetricKindsPagination>,
    _query_params: Query<ProjMetricKindsQuery>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Post.into()]))
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/metric-kinds",
    tags = ["projects", "metric kinds"]
}]
pub async fn proj_metric_kinds_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjMetricKindsParams>,
    pagination_params: Query<ProjMetricKindsPagination>,
    query_params: Query<ProjMetricKindsQuery>,
) -> Result<ResponseOk<JsonMetricKinds>, HttpError> {
    let auth_user = AuthUser::new_pub(&rqctx).await?;
    let json = get_ls_inner(
        rqctx.context(),
        auth_user.as_ref(),
        path_params.into_inner(),
        pagination_params.into_inner(),
        query_params.into_inner(),
    )
    .await?;
    Ok(Get::response_ok(json, auth_user.is_some()))
}

async fn get_ls_inner(
    context: &ApiContext,
    auth_user: Option<&AuthUser>,
    path_params: ProjMetricKindsParams,
    pagination_params: ProjMetricKindsPagination,
    query_params: ProjMetricKindsQuery,
) -> Result<JsonMetricKinds, HttpError> {
    let conn = &mut *context.conn().await;

    let query_project =
        QueryProject::is_allowed_public(conn, &context.rbac, &path_params.project, auth_user)?;

    let mut query = QueryMetricKind::belonging_to(&query_project).into_boxed();

    if let Some(name) = query_params.name.as_ref() {
        query = query.filter(schema::metric_kind::name.eq(name.as_ref()));
    }

    query = match pagination_params.order() {
        ProjMetricKindsSort::Name => match pagination_params.direction {
            Some(JsonDirection::Asc) | None => query.order(schema::metric_kind::name.asc()),
            Some(JsonDirection::Desc) => query.order(schema::metric_kind::name.desc()),
        },
    };

    let project = &query_project;
    Ok(query
        .offset(pagination_params.offset())
        .limit(pagination_params.limit())
        .load::<QueryMetricKind>(conn)
        .map_err(resource_not_found_err!(MetricKind, project))?
        .into_iter()
        .map(|metric_kind| metric_kind.into_json_for_project(project))
        .collect())
}

#[endpoint {
    method = POST,
    path =  "/v0/projects/{project}/metric-kinds",
    tags = ["projects", "metric kinds"]
}]
pub async fn proj_metric_kind_post(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<ProjMetricKindsParams>,
    body: TypedBody<JsonNewMetricKind>,
) -> Result<ResponseAccepted<JsonMetricKind>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = post_inner(
        rqctx.context(),
        path_params.into_inner(),
        body.into_inner(),
        &auth_user,
    )
    .await?;
    Ok(Post::auth_response_accepted(json))
}

async fn post_inner(
    context: &ApiContext,
    path_params: ProjMetricKindsParams,
    json_metric_kind: JsonNewMetricKind,
    auth_user: &AuthUser,
) -> Result<JsonMetricKind, HttpError> {
    let conn = &mut *context.conn().await;

    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed(
        conn,
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Create,
    )?;

    let insert_metric_kind = InsertMetricKind::from_json(conn, query_project.id, json_metric_kind)?;

    diesel::insert_into(schema::metric_kind::table)
        .values(&insert_metric_kind)
        .execute(conn)
        .map_err(resource_conflict_err!(MetricKind, insert_metric_kind))?;

    schema::metric_kind::table
        .filter(schema::metric_kind::uuid.eq(&insert_metric_kind.uuid))
        .first::<QueryMetricKind>(conn)
        .map(|metric_kind| metric_kind.into_json_for_project(&query_project))
        .map_err(resource_not_found_err!(MetricKind, insert_metric_kind))
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
    Ok(Endpoint::cors(&[Get.into(), Patch.into(), Delete.into()]))
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/metric-kinds/{metric_kind}",
    tags = ["projects", "metric kinds"]
}]
pub async fn proj_metric_kind_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubBearerToken,
    path_params: Path<ProjMetricKindParams>,
) -> Result<ResponseOk<JsonMetricKind>, HttpError> {
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
    path_params: ProjMetricKindParams,
    auth_user: Option<&AuthUser>,
) -> Result<JsonMetricKind, HttpError> {
    let conn = &mut *context.conn().await;

    let query_project =
        QueryProject::is_allowed_public(conn, &context.rbac, &path_params.project, auth_user)?;

    QueryMetricKind::belonging_to(&query_project)
        .filter(QueryMetricKind::resource_id(&path_params.metric_kind)?)
        .first::<QueryMetricKind>(conn)
        .map(|metric_kind| metric_kind.into_json_for_project(&query_project))
        .map_err(resource_not_found_err!(
            MetricKind,
            (&query_project, path_params.metric_kind)
        ))
}

#[endpoint {
    method = PATCH,
    path =  "/v0/projects/{project}/metric-kinds/{metric_kind}",
    tags = ["projects", "metric kinds"]
}]
pub async fn proj_metric_kind_patch(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<ProjMetricKindParams>,
    body: TypedBody<JsonUpdateMetricKind>,
) -> Result<ResponseAccepted<JsonMetricKind>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = patch_inner(
        rqctx.context(),
        path_params.into_inner(),
        body.into_inner(),
        &auth_user,
    )
    .await?;
    Ok(Patch::auth_response_accepted(json))
}

async fn patch_inner(
    context: &ApiContext,
    path_params: ProjMetricKindParams,
    json_metric_kind: JsonUpdateMetricKind,
    auth_user: &AuthUser,
) -> Result<JsonMetricKind, HttpError> {
    let conn = &mut *context.conn().await;

    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed(
        conn,
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Edit,
    )?;

    let query_metric_kind =
        QueryMetricKind::from_resource_id(conn, query_project.id, &path_params.metric_kind)?;

    diesel::update(
        schema::metric_kind::table.filter(schema::metric_kind::id.eq(query_metric_kind.id)),
    )
    .set(&UpdateMetricKind::from(json_metric_kind.clone()))
    .execute(conn)
    .map_err(resource_conflict_err!(
        MetricKind,
        (&query_metric_kind, &json_metric_kind)
    ))?;

    QueryMetricKind::get(conn, query_metric_kind.id)
        .map(|metric_kind| metric_kind.into_json_for_project(&query_project))
        .map_err(resource_not_found_err!(MetricKind, query_metric_kind))
}

#[endpoint {
    method = DELETE,
    path =  "/v0/projects/{project}/metric-kinds/{metric_kind}",
    tags = ["projects", "metric kinds"]
}]
pub async fn proj_metric_kind_delete(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<ProjMetricKindParams>,
) -> Result<ResponseAccepted<JsonEmpty>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = delete_inner(rqctx.context(), path_params.into_inner(), &auth_user).await?;
    Ok(Delete::auth_response_accepted(json))
}

async fn delete_inner(
    context: &ApiContext,
    path_params: ProjMetricKindParams,
    auth_user: &AuthUser,
) -> Result<JsonEmpty, HttpError> {
    let conn = &mut *context.conn().await;

    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed(
        conn,
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Delete,
    )?;

    let query_metric_kind =
        QueryMetricKind::from_resource_id(conn, query_project.id, &path_params.metric_kind)?;

    diesel::delete(
        schema::metric_kind::table.filter(schema::metric_kind::id.eq(query_metric_kind.id)),
    )
    .execute(conn)
    .map_err(resource_conflict_err!(MetricKind, query_metric_kind))?;

    Ok(JsonEmpty {})
}
