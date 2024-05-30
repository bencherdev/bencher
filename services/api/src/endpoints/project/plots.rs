use bencher_json::{
    project::plot::JsonUpdatePlot, JsonDirection, JsonNewPlot, JsonPagination, JsonPlot, JsonPlots,
    PlotUuid, ResourceId, ResourceName,
};
use bencher_rbac::project::Permission;
use diesel::{
    BelongingToDsl, BoolExpressionMethods, ExpressionMethods, NullableExpressionMethods, QueryDsl,
    RunQueryDsl, TextExpressionMethods,
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
    model::user::auth::{AuthUser, PubBearerToken},
    model::{
        project::{
            plot::{InsertPlot, QueryPlot, UpdatePlot},
            QueryProject,
        },
        user::auth::BearerToken,
    },
    schema,
    util::search::Search,
};

#[derive(Deserialize, JsonSchema)]
pub struct ProjPlotsParams {
    /// The slug or UUID for a project.
    pub project: ResourceId,
}

pub type ProjPlotsPagination = JsonPagination<ProjPlotsSort>;

#[derive(Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProjPlotsSort {
    /// Sort by plot rank.
    #[default]
    Rank,
}

#[derive(Deserialize, JsonSchema)]
pub struct ProjPlotsQuery {
    /// Filter by plot title, exact match.
    pub title: Option<ResourceName>,
    /// Search by plot title or UUID.
    pub search: Option<Search>,
}

#[allow(clippy::no_effect_underscore_binding, clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/plots",
    tags = ["projects", "plots"]
}]
pub async fn proj_plots_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjPlotsParams>,
    _pagination_params: Query<ProjPlotsPagination>,
    _query_params: Query<ProjPlotsQuery>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Post.into()]))
}

/// List plots for a project
///
/// List all plots for a project.
/// If the project is public, then the user does not need to be authenticated.
/// If the project is private, then the user must be authenticated and have `view` permissions for the project.
/// By default, the plots are sorted in their rank order.
#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/plots",
    tags = ["projects", "plots"]
}]
pub async fn proj_plots_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjPlotsParams>,
    pagination_params: Query<ProjPlotsPagination>,
    query_params: Query<ProjPlotsQuery>,
) -> Result<ResponseOk<JsonPlots>, HttpError> {
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
    path_params: ProjPlotsParams,
    pagination_params: ProjPlotsPagination,
    query_params: ProjPlotsQuery,
) -> Result<JsonPlots, HttpError> {
    let query_project = QueryProject::is_allowed_public(
        conn_lock!(context),
        &context.rbac,
        &path_params.project,
        auth_user,
    )?;

    let mut query = QueryPlot::belonging_to(&query_project).into_boxed();

    if let Some(title) = query_params.title.as_ref() {
        query = query.filter(schema::plot::title.nullable().eq(title));
    }
    if let Some(search) = query_params.search.as_ref() {
        query = query.filter(
            schema::plot::title
                .nullable()
                .like(search)
                .or(schema::plot::uuid.like(search)),
        );
    }

    query = match pagination_params.order() {
        ProjPlotsSort::Rank => match pagination_params.direction {
            Some(JsonDirection::Asc) | None => query.order(schema::plot::rank.asc()),
            Some(JsonDirection::Desc) => query.order(schema::plot::rank.desc()),
        },
    };

    conn_lock!(context, |conn| Ok(query
        .offset(pagination_params.offset())
        .limit(pagination_params.limit())
        .load::<QueryPlot>(conn)
        .map_err(resource_not_found_err!(Plot, &query_project))?
        .into_iter()
        .filter_map(
            |plot| match plot.into_json_for_project(conn, &query_project) {
                Ok(plot) => Some(plot),
                Err(err) => {
                    debug_assert!(false, "{err}");
                    #[cfg(feature = "sentry")]
                    sentry::capture_error(&err);
                    None
                },
            }
        )
        .collect()))
}

/// Create a plot
///
/// Create a plot for a project.
/// The user must have `manage` permissions for the project.
#[endpoint {
    method = POST,
    path =  "/v0/projects/{project}/plots",
    tags = ["projects", "plots"]
}]
pub async fn proj_plot_post(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<ProjPlotsParams>,
    body: TypedBody<JsonNewPlot>,
) -> Result<ResponseCreated<JsonPlot>, HttpError> {
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
    path_params: ProjPlotsParams,
    json_plot: JsonNewPlot,
    auth_user: &AuthUser,
) -> Result<JsonPlot, HttpError> {
    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed(
        conn_lock!(context),
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Manage,
    )?;

    let query_plot = InsertPlot::from_json(context, &query_project, json_plot).await?;

    query_plot.into_json_for_project(conn_lock!(context), &query_project)
}

#[derive(Deserialize, JsonSchema)]
pub struct ProjPlotParams {
    /// The slug or UUID for a project.
    pub project: ResourceId,
    /// The UUID for a plot.
    pub plot: PlotUuid,
}

#[allow(clippy::no_effect_underscore_binding, clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/plots/{plot}",
    tags = ["projects", "plots"]
}]
pub async fn proj_plot_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjPlotParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Patch.into(), Delete.into()]))
}

/// View a plot
///
/// View a plot for a project.
/// If the project is public, then the user does not need to be authenticated.
/// If the project is private, then the user must be authenticated and have `view` permissions for the project.
#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/plots/{plot}",
    tags = ["projects", "plots"]
}]
pub async fn proj_plot_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubBearerToken,
    path_params: Path<ProjPlotParams>,
) -> Result<ResponseOk<JsonPlot>, HttpError> {
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
    path_params: ProjPlotParams,
    auth_user: Option<&AuthUser>,
) -> Result<JsonPlot, HttpError> {
    let query_project = QueryProject::is_allowed_public(
        conn_lock!(context),
        &context.rbac,
        &path_params.project,
        auth_user,
    )?;

    conn_lock!(context, |conn| QueryPlot::get_with_uuid(
        conn,
        &query_project,
        path_params.plot
    )
    .and_then(|plot| plot.into_json_for_project(conn, &query_project)))
}

/// Update a plot
///
/// Update a plot for a project.
/// The user must have `manage` permissions for the project.
#[endpoint {
    method = PATCH,
    path =  "/v0/projects/{project}/plots/{plot}",
    tags = ["projects", "plots"]
}]
pub async fn proj_plot_patch(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<ProjPlotParams>,
    body: TypedBody<JsonUpdatePlot>,
) -> Result<ResponseOk<JsonPlot>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let context = rqctx.context();
    let json = patch_inner(
        context,
        path_params.into_inner(),
        body.into_inner(),
        &auth_user,
    )
    .await?;
    Ok(Patch::auth_response_ok(json))
}

async fn patch_inner(
    context: &ApiContext,
    path_params: ProjPlotParams,
    json_plot: JsonUpdatePlot,
    auth_user: &AuthUser,
) -> Result<JsonPlot, HttpError> {
    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed(
        conn_lock!(context),
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Manage,
    )?;

    let query_plot =
        QueryPlot::get_with_uuid(conn_lock!(context), &query_project, path_params.plot)?;

    let update_plot =
        UpdatePlot::from_json(context, &query_project, &query_plot, json_plot.clone()).await?;
    diesel::update(schema::plot::table.filter(schema::plot::id.eq(query_plot.id)))
        .set(&update_plot)
        .execute(conn_lock!(context))
        .map_err(resource_conflict_err!(Plot, (&query_plot, &json_plot)))?;

    conn_lock!(context, |conn| QueryPlot::get_with_uuid(
        conn,
        &query_project,
        path_params.plot
    )
    .and_then(|plot| plot.into_json_for_project(conn, &query_project)))
}

/// Delete a plot
///
/// Delete a plot for a project.
/// The user must have `delete` permissions for the project.
/// This plot will no longer appear in the project dashboard.
#[endpoint {
    method = DELETE,
    path =  "/v0/projects/{project}/plots/{plot}",
    tags = ["projects", "plots"]
}]
pub async fn proj_plot_delete(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<ProjPlotParams>,
) -> Result<ResponseDeleted, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    delete_inner(rqctx.context(), path_params.into_inner(), &auth_user).await?;
    Ok(Delete::auth_response_deleted())
}

async fn delete_inner(
    context: &ApiContext,
    path_params: ProjPlotParams,
    auth_user: &AuthUser,
) -> Result<(), HttpError> {
    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed(
        conn_lock!(context),
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Manage,
    )?;

    let query_plot =
        QueryPlot::get_with_uuid(conn_lock!(context), &query_project, path_params.plot)?;

    diesel::delete(schema::plot::table.filter(schema::plot::id.eq(query_plot.id)))
        .execute(conn_lock!(context))
        .map_err(resource_conflict_err!(Plot, query_plot))?;

    Ok(())
}
