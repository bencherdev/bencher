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
    conn_lock,
    context::ApiContext,
    endpoints::{
        endpoint::{
            CorsResponse, Delete, Get, Patch, Post, ResponseCreated, ResponseDeleted, ResponseOk,
        },
        Endpoint,
    },
    error::{resource_conflict_err, resource_not_found_err},
    model::{
        project::{
            measure::{InsertMeasure, QueryMeasure, UpdateMeasure},
            QueryProject,
        },
        user::auth::{AuthUser, BearerToken, PubBearerToken},
    },
    schema,
    util::{headers::TotalCount, search::Search},
};

#[derive(Deserialize, JsonSchema)]
pub struct ProjMeasuresParams {
    /// The slug or UUID for a project.
    pub project: ResourceId,
}

pub type ProjMeasuresPagination = JsonPagination<ProjMeasuresSort>;

#[derive(Debug, Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProjMeasuresSort {
    /// Sort by measure name.
    #[default]
    Name,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProjMeasuresQuery {
    /// Filter by measure name, exact match.
    pub name: Option<ResourceName>,
    /// Search by measure name, slug, or UUID.
    pub search: Option<Search>,
    /// If set to `true`, only returns archived measures if set to `true`.
    /// If not set or set to `false`, only returns non-archived measures.
    pub archived: Option<bool>,
}

#[allow(clippy::no_effect_underscore_binding, clippy::unused_async)]
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

/// List measures for a project
///
/// List all measures for a project.
/// If the project is public, then the user does not need to be authenticated.
/// If the project is private, then the user must be authenticated and have `view` permissions for the project.
/// By default, the measures are sorted in alphabetical order by name.
/// The HTTP response header `X-Total-Count` contains the total number of measures.
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
    let (json, total_count) = get_ls_inner(
        rqctx.context(),
        auth_user.as_ref(),
        path_params.into_inner(),
        pagination_params.into_inner(),
        query_params.into_inner(),
    )
    .await?;
    Ok(Get::response_ok_with_total_count(
        json,
        auth_user.is_some(),
        total_count,
    ))
}

async fn get_ls_inner(
    context: &ApiContext,
    auth_user: Option<&AuthUser>,
    path_params: ProjMeasuresParams,
    pagination_params: ProjMeasuresPagination,
    query_params: ProjMeasuresQuery,
) -> Result<(JsonMeasures, TotalCount), HttpError> {
    let query_project = QueryProject::is_allowed_public(
        conn_lock!(context),
        &context.rbac,
        &path_params.project,
        auth_user,
    )?;

    let measures = get_ls_query(&query_project, &pagination_params, &query_params)
        .offset(pagination_params.offset())
        .limit(pagination_params.limit())
        .load::<QueryMeasure>(conn_lock!(context))
        .map_err(resource_not_found_err!(
            Measure,
            (&query_project, &pagination_params, &query_params)
        ))?;

    // Drop connection lock before iterating
    let json_measures = measures
        .into_iter()
        .map(|measure| measure.into_json_for_project(&query_project))
        .collect();

    let total_count = get_ls_query(&query_project, &pagination_params, &query_params)
        .count()
        .get_result::<i64>(conn_lock!(context))
        .map_err(resource_not_found_err!(
            Measure,
            (&query_project, &pagination_params, &query_params)
        ))?
        .try_into()?;

    Ok((json_measures, total_count))
}

fn get_ls_query<'q>(
    query_project: &'q QueryProject,
    pagination_params: &ProjMeasuresPagination,
    query_params: &'q ProjMeasuresQuery,
) -> schema::measure::BoxedQuery<'q, diesel::sqlite::Sqlite> {
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

    if let Some(true) = query_params.archived {
        query = query.filter(schema::measure::archived.is_not_null());
    } else {
        query = query.filter(schema::measure::archived.is_null());
    };

    match pagination_params.order() {
        ProjMeasuresSort::Name => match pagination_params.direction {
            Some(JsonDirection::Asc) | None => query.order(schema::measure::name.asc()),
            Some(JsonDirection::Desc) => query.order(schema::measure::name.desc()),
        },
    }
}

/// Create a measure
///
/// Create a measure for a project.
/// The user must have `create` permissions for the project.
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
    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed(
        conn_lock!(context),
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Create,
    )?;

    let insert_measure =
        InsertMeasure::from_json(conn_lock!(context), query_project.id, json_measure)?;

    diesel::insert_into(schema::measure::table)
        .values(&insert_measure)
        .execute(conn_lock!(context))
        .map_err(resource_conflict_err!(Measure, insert_measure))?;

    schema::measure::table
        .filter(schema::measure::uuid.eq(&insert_measure.uuid))
        .first::<QueryMeasure>(conn_lock!(context))
        .map(|measure| measure.into_json_for_project(&query_project))
        .map_err(resource_not_found_err!(Measure, insert_measure))
}

#[derive(Deserialize, JsonSchema)]
pub struct ProjMeasureParams {
    /// The slug or UUID for a project.
    pub project: ResourceId,
    /// The slug or UUID for a measure.
    pub measure: ResourceId,
}

#[allow(clippy::no_effect_underscore_binding, clippy::unused_async)]
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

/// View a measure
///
/// View a measure for a project.
/// If the project is public, then the user does not need to be authenticated.
/// If the project is private, then the user must be authenticated and have `view` permissions for the project.
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
    let query_project = QueryProject::is_allowed_public(
        conn_lock!(context),
        &context.rbac,
        &path_params.project,
        auth_user,
    )?;

    QueryMeasure::belonging_to(&query_project)
        .filter(QueryMeasure::eq_resource_id(&path_params.measure)?)
        .first::<QueryMeasure>(conn_lock!(context))
        .map(|measure| measure.into_json_for_project(&query_project))
        .map_err(resource_not_found_err!(
            Measure,
            (&query_project, path_params.measure)
        ))
}

/// Update a measure
///
/// Update a measure for a project.
/// The user must have `edit` permissions for the project.
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
    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed(
        conn_lock!(context),
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Edit,
    )?;

    let query_measure = QueryMeasure::from_resource_id(
        conn_lock!(context),
        query_project.id,
        &path_params.measure,
    )?;
    let update_measure = UpdateMeasure::from(json_measure.clone());
    diesel::update(schema::measure::table.filter(schema::measure::id.eq(query_measure.id)))
        .set(&update_measure)
        .execute(conn_lock!(context))
        .map_err(resource_conflict_err!(
            Measure,
            (&query_measure, &json_measure)
        ))?;

    QueryMeasure::get(conn_lock!(context), query_measure.id)
        .map(|measure| measure.into_json_for_project(&query_project))
        .map_err(resource_not_found_err!(Measure, query_measure))
}

/// Delete a measure
///
/// Delete a measure for a project.
/// The user must have `delete` permissions for the project.
/// All reports and thresholds that use this measure must be deleted first!
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
    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed(
        conn_lock!(context),
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Delete,
    )?;

    let query_measure = QueryMeasure::from_resource_id(
        conn_lock!(context),
        query_project.id,
        &path_params.measure,
    )?;

    diesel::delete(schema::measure::table.filter(schema::measure::id.eq(query_measure.id)))
        .execute(conn_lock!(context))
        .map_err(resource_conflict_err!(Measure, query_measure))?;

    Ok(())
}
