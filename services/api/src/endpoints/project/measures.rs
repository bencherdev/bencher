use bencher_json::{
    project::measure::JsonUpdateMeasure, JsonDirection, JsonMeasure, JsonMeasures, JsonNewMeasure,
    JsonPagination, ResourceId, ResourceName,
};
use bencher_rbac::project::Permission;
use diesel::{
    BelongingToDsl, BoolExpressionMethods, ExpressionMethods, QueryDsl, RunQueryDsl,
    TextExpressionMethods,
};
use dropshot::{endpoint, HttpError, Path, Query, RequestContext, TypedBody};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    context::ApiContext,
    endpoints::{
        endpoint::{
            CorsResponse, Delete, Get, Patch, Post, ResponseCreated, ResponseDeleted, ResponseOk,
        },
        Endpoint,
    },
    error::{resource_conflict_err, resource_not_found_err},
    model::user::auth::{AuthUser, PubBearerToken},
    model::{
        project::{
            measure::{InsertMeasure, QueryMeasure, UpdateMeasure},
            QueryProject,
        },
        user::auth::BearerToken,
    },
    schema,
    util::search::Search,
};

#[derive(Deserialize, JsonSchema)]
pub struct ProjMeasuresParams {
    pub project: ResourceId,
}

pub type ProjMeasuresPagination = JsonPagination<ProjMeasuresSort>;

#[derive(Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProjMeasuresSort {
    #[default]
    Name,
}

#[derive(Deserialize, JsonSchema)]
pub struct ProjMeasuresQuery {
    pub name: Option<ResourceName>,
    pub search: Option<Search>,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/measures",
    tags = ["projects", "measures"]
}]
pub async fn proj_measures_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjMeasuresParams>,
    _pagination_params: Query<ProjMeasuresPagination>,
    _query_params: Query<ProjMeasuresQuery>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Post.into()]))
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/measures",
    tags = ["projects", "measures"]
}]
pub async fn proj_measures_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjMeasuresParams>,
    pagination_params: Query<ProjMeasuresPagination>,
    query_params: Query<ProjMeasuresQuery>,
) -> Result<ResponseOk<JsonMeasures>, HttpError> {
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
    path_params: ProjMeasuresParams,
    pagination_params: ProjMeasuresPagination,
    query_params: ProjMeasuresQuery,
) -> Result<JsonMeasures, HttpError> {
    let conn = &mut *context.conn().await;

    let query_project =
        QueryProject::is_allowed_public(conn, &context.rbac, &path_params.project, auth_user)?;

    let mut query = QueryMeasure::belonging_to(&query_project).into_boxed();

    if let Some(name) = query_params.name.as_ref() {
        query = query.filter(schema::measure::name.eq(name));
    }
    if let Some(search) = query_params.search.as_ref() {
        query = query.filter(
            schema::measure::name
                .like(search)
                .or(schema::measure::slug.like(search))
                .or(schema::measure::uuid.like(search)),
        );
    }

    query = match pagination_params.order() {
        ProjMeasuresSort::Name => match pagination_params.direction {
            Some(JsonDirection::Asc) | None => query.order(schema::measure::name.asc()),
            Some(JsonDirection::Desc) => query.order(schema::measure::name.desc()),
        },
    };

    let project = &query_project;
    Ok(query
        .offset(pagination_params.offset())
        .limit(pagination_params.limit())
        .load::<QueryMeasure>(conn)
        .map_err(resource_not_found_err!(Measure, project))?
        .into_iter()
        .map(|measure| measure.into_json_for_project(project))
        .collect())
}

#[endpoint {
    method = POST,
    path =  "/v0/projects/{project}/measures",
    tags = ["projects", "measures"]
}]
pub async fn proj_measure_post(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<ProjMeasuresParams>,
    body: TypedBody<JsonNewMeasure>,
) -> Result<ResponseCreated<JsonMeasure>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = post_inner(
        rqctx.context(),
        path_params.into_inner(),
        body.into_inner(),
        &auth_user,
    )
    .await?;
    Ok(Post::auth_response_created(json))
}

async fn post_inner(
    context: &ApiContext,
    path_params: ProjMeasuresParams,
    json_measure: JsonNewMeasure,
    auth_user: &AuthUser,
) -> Result<JsonMeasure, HttpError> {
    let conn = &mut *context.conn().await;

    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed(
        conn,
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Create,
    )?;

    let insert_measure = InsertMeasure::from_json(conn, query_project.id, json_measure)?;

    diesel::insert_into(schema::measure::table)
        .values(&insert_measure)
        .execute(conn)
        .map_err(resource_conflict_err!(Measure, insert_measure))?;

    schema::measure::table
        .filter(schema::measure::uuid.eq(&insert_measure.uuid))
        .first::<QueryMeasure>(conn)
        .map(|measure| measure.into_json_for_project(&query_project))
        .map_err(resource_not_found_err!(Measure, insert_measure))
}

#[derive(Deserialize, JsonSchema)]
pub struct ProjMeasureParams {
    pub project: ResourceId,
    pub measure: ResourceId,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/measures/{measure}",
    tags = ["projects", "measures"]
}]
pub async fn proj_measure_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjMeasureParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Patch.into(), Delete.into()]))
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/measures/{measure}",
    tags = ["projects", "measures"]
}]
pub async fn proj_measure_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubBearerToken,
    path_params: Path<ProjMeasureParams>,
) -> Result<ResponseOk<JsonMeasure>, HttpError> {
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
    path_params: ProjMeasureParams,
    auth_user: Option<&AuthUser>,
) -> Result<JsonMeasure, HttpError> {
    let conn = &mut *context.conn().await;

    let query_project =
        QueryProject::is_allowed_public(conn, &context.rbac, &path_params.project, auth_user)?;

    QueryMeasure::belonging_to(&query_project)
        .filter(QueryMeasure::eq_resource_id(&path_params.measure)?)
        .first::<QueryMeasure>(conn)
        .map(|measure| measure.into_json_for_project(&query_project))
        .map_err(resource_not_found_err!(
            Measure,
            (&query_project, path_params.measure)
        ))
}

#[endpoint {
    method = PATCH,
    path =  "/v0/projects/{project}/measures/{measure}",
    tags = ["projects", "measures"]
}]
pub async fn proj_measure_patch(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<ProjMeasureParams>,
    body: TypedBody<JsonUpdateMeasure>,
) -> Result<ResponseOk<JsonMeasure>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = patch_inner(
        rqctx.context(),
        path_params.into_inner(),
        body.into_inner(),
        &auth_user,
    )
    .await?;
    Ok(Patch::auth_response_ok(json))
}

async fn patch_inner(
    context: &ApiContext,
    path_params: ProjMeasureParams,
    json_measure: JsonUpdateMeasure,
    auth_user: &AuthUser,
) -> Result<JsonMeasure, HttpError> {
    let conn = &mut *context.conn().await;

    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed(
        conn,
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Edit,
    )?;

    let query_measure =
        QueryMeasure::from_resource_id(conn, query_project.id, &path_params.measure)?;

    diesel::update(schema::measure::table.filter(schema::measure::id.eq(query_measure.id)))
        .set(&UpdateMeasure::from(json_measure.clone()))
        .execute(conn)
        .map_err(resource_conflict_err!(
            Measure,
            (&query_measure, &json_measure)
        ))?;

    QueryMeasure::get(conn, query_measure.id)
        .map(|measure| measure.into_json_for_project(&query_project))
        .map_err(resource_not_found_err!(Measure, query_measure))
}

#[endpoint {
    method = DELETE,
    path =  "/v0/projects/{project}/measures/{measure}",
    tags = ["projects", "measures"]
}]
pub async fn proj_measure_delete(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<ProjMeasureParams>,
) -> Result<ResponseDeleted, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    delete_inner(rqctx.context(), path_params.into_inner(), &auth_user).await?;
    Ok(Delete::auth_response_deleted())
}

async fn delete_inner(
    context: &ApiContext,
    path_params: ProjMeasureParams,
    auth_user: &AuthUser,
) -> Result<(), HttpError> {
    let conn = &mut *context.conn().await;

    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed(
        conn,
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Delete,
    )?;

    let query_measure =
        QueryMeasure::from_resource_id(conn, query_project.id, &path_params.measure)?;

    diesel::delete(schema::measure::table.filter(schema::measure::id.eq(query_measure.id)))
        .execute(conn)
        .map_err(resource_conflict_err!(Measure, query_measure))?;

    Ok(())
}
