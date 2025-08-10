use bencher_endpoint::{
    CorsResponse, Delete, Endpoint, Get, Patch, Post, ResponseCreated, ResponseDeleted, ResponseOk,
    TotalCount,
};
use bencher_json::{
    JsonDirection, JsonNewPlot, JsonPagination, JsonPlot, JsonPlots, PlotUuid, ProjectResourceId,
    ResourceName, Search, project::plot::JsonUpdatePlot,
};
use bencher_rbac::project::Permission;
use bencher_schema::{
    conn_lock,
    context::ApiContext,
    error::{resource_conflict_err, resource_not_found_err},
    model::{
        project::{
            QueryProject,
            plot::{InsertPlot, QueryPlot, UpdatePlot},
        },
        user::auth::{AuthUser, BearerToken, PubBearerToken},
    },
    schema,
};
use diesel::{
    BelongingToDsl as _, BoolExpressionMethods as _, ExpressionMethods as _,
    NullableExpressionMethods as _, QueryDsl as _, RunQueryDsl as _, TextExpressionMethods as _,
};
use dropshot::{HttpError, Path, Query, RequestContext, TypedBody, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, JsonSchema)]
pub struct ProjPlotsParams {
    /// The slug or UUID for a project.
    pub project: ProjectResourceId,
}

pub type ProjPlotsPagination = JsonPagination<ProjPlotsSort>;

#[derive(Debug, Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProjPlotsSort {
    /// Sort by plot index.
    #[default]
    Index,
    /// Sort by plot title.
    Title,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProjPlotsQuery {
    /// Filter by plot title, exact match.
    pub title: Option<ResourceName>,
    /// Search by plot title or UUID.
    pub search: Option<Search>,
}

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
/// By default, the plots are sorted in their index order.
/// The HTTP response header `X-Total-Count` contains the total number of plots.
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
    path_params: ProjPlotsParams,
    pagination_params: ProjPlotsPagination,
    query_params: ProjPlotsQuery,
) -> Result<(JsonPlots, TotalCount), HttpError> {
    let query_project = QueryProject::is_allowed_public(
        conn_lock!(context),
        &context.rbac,
        &path_params.project,
        auth_user,
    )?;

    let plots = get_ls_query(&query_project, &pagination_params, &query_params)
        .offset(pagination_params.offset())
        .limit(pagination_params.limit())
        .load::<QueryPlot>(conn_lock!(context))
        .map_err(resource_not_found_err!(
            Plot,
            (&query_project, &pagination_params, &query_params)
        ))?;

    // Separate out these queries to prevent a deadlock when getting the conn_lock
    let mut json_plots = Vec::with_capacity(plots.len());
    for plot in plots {
        match plot.into_json_for_project(conn_lock!(context), &query_project) {
            Ok(plot) => json_plots.push(plot),
            Err(err) => {
                debug_assert!(false, "{err}");
                #[cfg(feature = "sentry")]
                sentry::capture_error(&err);
            },
        }
    }

    let total_count = get_ls_query(&query_project, &pagination_params, &query_params)
        .count()
        .get_result::<i64>(conn_lock!(context))
        .map_err(resource_not_found_err!(
            Plot,
            (&query_project, &pagination_params, &query_params)
        ))?
        .try_into()?;

    Ok((json_plots.into(), total_count))
}

fn get_ls_query<'q>(
    query_project: &'q QueryProject,
    pagination_params: &ProjPlotsPagination,
    query_params: &'q ProjPlotsQuery,
) -> schema::plot::BoxedQuery<'q, diesel::sqlite::Sqlite> {
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

    match pagination_params.order() {
        ProjPlotsSort::Index => match pagination_params.direction {
            Some(JsonDirection::Asc) | None => query.order(schema::plot::rank.asc()),
            Some(JsonDirection::Desc) => query.order(schema::plot::rank.desc()),
        },
        ProjPlotsSort::Title => match pagination_params.direction {
            Some(JsonDirection::Asc) | None => query.order(schema::plot::title.asc()),
            Some(JsonDirection::Desc) => query.order(schema::plot::title.desc()),
        },
    }
}

/// Create a plot
///
/// Create a plot for a project.
/// The user must have `create` permissions for the project.
/// A project can have a maximum of 64 plots at a time.
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
        Permission::Create,
    )?;

    #[cfg(feature = "plus")]
    InsertPlot::rate_limit(context, query_project.id).await?;
    let query_plot = InsertPlot::from_json(context, &query_project, json_plot).await?;

    query_plot.into_json_for_project(conn_lock!(context), &query_project)
}

#[derive(Deserialize, JsonSchema)]
pub struct ProjPlotParams {
    /// The slug or UUID for a project.
    pub project: ProjectResourceId,
    /// The UUID for a plot.
    pub plot: PlotUuid,
}

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
/// The user must have `edit` permissions for the project.
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
        Permission::Edit,
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
        Permission::Delete,
    )?;

    let query_plot =
        QueryPlot::get_with_uuid(conn_lock!(context), &query_project, path_params.plot)?;

    diesel::delete(schema::plot::table.filter(schema::plot::id.eq(query_plot.id)))
        .execute(conn_lock!(context))
        .map_err(resource_conflict_err!(Plot, query_plot))?;

    Ok(())
}
