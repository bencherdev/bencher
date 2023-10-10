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
        endpoint::{
            pub_response_ok, response_accepted, response_ok, CorsResponse, ResponseAccepted,
            ResponseOk,
        },
        Endpoint,
    },
    model::project::{
        metric_kind::{InsertMetricKind, QueryMetricKind, UpdateMetricKind},
        QueryProject,
    },
    model::user::auth::AuthUser,
    schema,
    util::{error::into_json, resource_id::fn_resource_id},
    ApiError,
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
    Ok(Endpoint::cors(&[Endpoint::GetLs, Endpoint::Post]))
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
    let auth_user = AuthUser::new(&rqctx).await.ok();
    let endpoint = Endpoint::GetLs;

    let json = get_ls_inner(
        rqctx.context(),
        auth_user.as_ref(),
        path_params.into_inner(),
        pagination_params.into_inner(),
        query_params.into_inner(),
        endpoint,
    )
    .await
    .map_err(|e| {
        if let ApiError::HttpError(e) = e {
            e
        } else {
            endpoint.err(e).into()
        }
    })?;

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
    pagination_params: ProjMetricKindsPagination,
    query_params: ProjMetricKindsQuery,
    endpoint: Endpoint,
) -> Result<JsonMetricKinds, ApiError> {
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

    Ok(query
        .offset(pagination_params.offset())
        .limit(pagination_params.limit())
        .load::<QueryMetricKind>(conn)
        .map_err(ApiError::from)?
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
    let endpoint = Endpoint::Post;

    let json = post_inner(
        rqctx.context(),
        path_params.into_inner(),
        body.into_inner(),
        &auth_user,
    )
    .await
    .map_err(|e| {
        if let ApiError::HttpError(e) = e {
            e
        } else {
            endpoint.err(e).into()
        }
    })?;

    response_accepted!(endpoint, json)
}

async fn post_inner(
    context: &ApiContext,
    path_params: ProjMetricKindsParams,
    json_metric_kind: JsonNewMetricKind,
    auth_user: &AuthUser,
) -> Result<JsonMetricKind, ApiError> {
    let conn = &mut *context.conn().await;

    // Verify that the user is allowed
    let project_id = QueryProject::is_allowed(
        conn,
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Create,
    )?
    .id;

    let insert_metric_kind = InsertMetricKind::from_json(conn, project_id, json_metric_kind);

    // This check is required because not all system metric kinds are created at project init
    // And default metric kinds may be deleted
    if insert_metric_kind.is_system() {
        return Err(ApiError::SystemMetricKind);
    }
    diesel::insert_into(schema::metric_kind::table)
        .values(&insert_metric_kind)
        .execute(conn)
        .map_err(ApiError::from)?;

    schema::metric_kind::table
        .filter(schema::metric_kind::uuid.eq(&insert_metric_kind.uuid))
        .first::<QueryMetricKind>(conn)
        .map_err(ApiError::from)?
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
    Ok(Endpoint::cors(&[
        Endpoint::GetOne,
        Endpoint::Patch,
        Endpoint::Delete,
    ]))
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
    let endpoint = Endpoint::GetOne;

    let json = get_one_inner(
        rqctx.context(),
        path_params.into_inner(),
        auth_user.as_ref(),
    )
    .await
    .map_err(|e| {
        if let ApiError::HttpError(e) = e {
            e
        } else {
            endpoint.err(e).into()
        }
    })?;

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

    QueryMetricKind::belonging_to(&query_project)
        .filter(resource_id(&path_params.metric_kind)?)
        .first::<QueryMetricKind>(conn)
        .map_err(ApiError::from)?
        .into_json(conn)
}

#[endpoint {
    method = PATCH,
    path =  "/v0/projects/{project}/metric-kinds/{metric_kind}",
    tags = ["projects", "metric kinds"]
}]
pub async fn proj_metric_kind_patch(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjMetricKindParams>,
    body: TypedBody<JsonUpdateMetricKind>,
) -> Result<ResponseAccepted<JsonMetricKind>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::Patch;

    let context = rqctx.context();
    let json = patch_inner(
        context,
        path_params.into_inner(),
        body.into_inner(),
        &auth_user,
    )
    .await
    .map_err(|e| {
        if let ApiError::HttpError(e) = e {
            e
        } else {
            endpoint.err(e).into()
        }
    })?;

    response_accepted!(endpoint, json)
}

async fn patch_inner(
    context: &ApiContext,
    path_params: ProjMetricKindParams,
    json_metric_kind: JsonUpdateMetricKind,
    auth_user: &AuthUser,
) -> Result<JsonMetricKind, ApiError> {
    let conn = &mut *context.conn().await;

    // Verify that the user is allowed
    let project_id = QueryProject::is_allowed(
        conn,
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Edit,
    )?
    .id;

    let query_metric_kind =
        QueryMetricKind::from_resource_id(conn, project_id, &path_params.metric_kind)?;
    if query_metric_kind.is_system() {
        return Err(ApiError::SystemMetricKind);
    }
    diesel::update(
        schema::metric_kind::table.filter(schema::metric_kind::id.eq(query_metric_kind.id)),
    )
    .set(&UpdateMetricKind::from(json_metric_kind))
    .execute(conn)
    .map_err(ApiError::from)?;

    QueryMetricKind::get(conn, query_metric_kind.id)?.into_json(conn)
}

#[endpoint {
    method = DELETE,
    path =  "/v0/projects/{project}/metric-kinds/{metric_kind}",
    tags = ["projects", "metric kinds"]
}]
pub async fn proj_metric_kind_delete(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjMetricKindParams>,
) -> Result<ResponseAccepted<JsonEmpty>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::Delete;

    let json = delete_inner(rqctx.context(), path_params.into_inner(), &auth_user)
        .await
        .map_err(|e| {
            if let ApiError::HttpError(e) = e {
                e
            } else {
                endpoint.err(e).into()
            }
        })?;

    response_accepted!(endpoint, json)
}

async fn delete_inner(
    context: &ApiContext,
    path_params: ProjMetricKindParams,
    auth_user: &AuthUser,
) -> Result<JsonEmpty, ApiError> {
    let conn = &mut *context.conn().await;

    // Verify that the user is allowed
    let project_id = QueryProject::is_allowed(
        conn,
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Delete,
    )?
    .id;

    let query_metric_kind =
        QueryMetricKind::from_resource_id(conn, project_id, &path_params.metric_kind)?;

    diesel::delete(
        schema::metric_kind::table.filter(schema::metric_kind::id.eq(query_metric_kind.id)),
    )
    .execute(conn)
    .map_err(ApiError::from)?;

    Ok(JsonEmpty {})
}
