use bencher_json::{
    project::{JsonUpdateProject, Visibility},
    JsonDirection, JsonPagination, JsonProject, JsonProjects, ResourceId, ResourceName,
};
use bencher_rbac::project::Permission;
use diesel::{
    BoolExpressionMethods, ExpressionMethods, QueryDsl, RunQueryDsl, TextExpressionMethods,
};
use dropshot::{endpoint, HttpError, Path, Query, RequestContext, TypedBody};
use schemars::JsonSchema;
use serde::Deserialize;
use slog::Logger;

use crate::{
    conn_lock,
    context::ApiContext,
    endpoints::{
        endpoint::{CorsResponse, Delete, Get, Patch, ResponseDeleted, ResponseOk},
        Endpoint,
    },
    error::{resource_conflict_err, resource_not_found_err},
    model::{
        project::{QueryProject, UpdateProject},
        user::auth::{AuthUser, BearerToken, PubBearerToken},
    },
    schema,
    util::search::Search,
};

pub type ProjectsPagination = JsonPagination<ProjectsSort>;

#[derive(Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProjectsSort {
    /// Sort by project name.
    #[default]
    Name,
}

#[derive(Clone, Deserialize, JsonSchema)]
pub struct ProjectsQuery {
    /// Filter by project name, exact match.
    pub name: Option<ResourceName>,
    /// Search by project name, slug, or UUID.
    pub search: Option<Search>,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects",
    tags = ["projects"]
}]
pub async fn projects_options(
    _rqctx: RequestContext<ApiContext>,
    _pagination_params: Query<ProjectsPagination>,
    _query_params: Query<ProjectsQuery>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

/// List projects
///
/// List all projects.
/// If the user is not authenticated, then only public projects are returned.
/// If the user is authenticated, then all public projects and
/// any private project where the user has `view` permissions are returned.
/// By default, the projects are sorted in alphabetical order by name.
#[endpoint {
    method = GET,
    path =  "/v0/projects",
    tags = ["projects"]
}]
pub async fn projects_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubBearerToken,
    pagination_params: Query<ProjectsPagination>,
    query_params: Query<ProjectsQuery>,
) -> Result<ResponseOk<JsonProjects>, HttpError> {
    let auth_user = AuthUser::from_pub_token(rqctx.context(), bearer_token).await?;
    let json = get_ls_inner(
        rqctx.context(),
        auth_user.as_ref(),
        pagination_params.into_inner(),
        query_params.into_inner(),
    )
    .await?;
    Ok(Get::response_ok(json, auth_user.is_some()))
}

async fn get_ls_inner(
    context: &ApiContext,
    auth_user: Option<&AuthUser>,
    pagination_params: ProjectsPagination,
    query_params: ProjectsQuery,
) -> Result<JsonProjects, HttpError> {
    let mut query = schema::project::table.into_boxed();

    // All users should just see the public projects if the query is for public projects
    if let Some(auth_user) = auth_user {
        if !auth_user.is_admin(&context.rbac) {
            let projects =
                auth_user.projects(&context.rbac, bencher_rbac::project::Permission::View);
            query = query.filter(
                schema::project::id
                    .eq_any(projects)
                    .or(schema::project::visibility.eq(Visibility::Public)),
            );
        }
    } else {
        query = query.filter(schema::project::visibility.eq(Visibility::Public));
    }

    if let Some(name) = query_params.name.as_ref() {
        query = query.filter(schema::project::name.eq(name));
    }
    if let Some(search) = query_params.search.as_ref() {
        query = query.filter(
            schema::project::name
                .like(search)
                .or(schema::project::slug.like(search))
                .or(schema::project::uuid.like(search)),
        );
    }

    query = match pagination_params.order() {
        ProjectsSort::Name => match pagination_params.direction {
            Some(JsonDirection::Asc) | None => query.order(schema::project::name.asc()),
            Some(JsonDirection::Desc) => query.order(schema::project::name.desc()),
        },
    };

    conn_lock!(context, |conn| Ok(query
        .offset(pagination_params.offset())
        .limit(pagination_params.limit())
        .load::<QueryProject>(conn)
        .map_err(resource_not_found_err!(Project))?
        .into_iter()
        .filter_map(|project| match project.into_json(conn) {
            Ok(project) => Some(project),
            Err(err) => {
                debug_assert!(false, "{err}");
                #[cfg(feature = "sentry")]
                sentry::capture_error(&err);
                None
            },
        })
        .collect()))
}

#[derive(Deserialize, JsonSchema)]
pub struct ProjectParams {
    /// The slug or UUID for a project.
    pub project: ResourceId,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}",
    tags = ["projects"]
}]
pub async fn project_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjectParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Patch.into(), Delete.into()]))
}

/// View a project
///
/// View a project.
/// If the user is not authenticated, then only a public project is available.
/// If the user is authenticated, then any public project and
/// any private project where the user has `view` permissions is available.
#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}",
    tags = [ "projects"]
}]
pub async fn project_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubBearerToken,
    path_params: Path<ProjectParams>,
) -> Result<ResponseOk<JsonProject>, HttpError> {
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
    path_params: ProjectParams,
    auth_user: Option<&AuthUser>,
) -> Result<JsonProject, HttpError> {
    conn_lock!(context, |conn| QueryProject::is_allowed_public(
        conn,
        &context.rbac,
        &path_params.project,
        auth_user
    )?
    .into_json(conn))
}

/// Update a project
///
/// Update a project.
/// The user must have `edit` permissions for the project.
#[endpoint {
    method = PATCH,
    path =  "/v0/projects/{project}",
    tags = ["projects"]
}]
pub async fn project_patch(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<ProjectParams>,
    body: TypedBody<JsonUpdateProject>,
) -> Result<ResponseOk<JsonProject>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let context = rqctx.context();
    let json = patch_inner(
        &rqctx.log,
        context,
        path_params.into_inner(),
        body.into_inner(),
        &auth_user,
    )
    .await?;
    Ok(Patch::auth_response_ok(json))
}

async fn patch_inner(
    log: &Logger,
    context: &ApiContext,
    path_params: ProjectParams,
    json_project: JsonUpdateProject,
    auth_user: &AuthUser,
) -> Result<JsonProject, HttpError> {
    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed(
        conn_lock!(context),
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Edit,
    )?;

    // Check project visibility
    #[cfg(not(feature = "plus"))]
    QueryProject::is_visibility_public(json_project.visibility())?;
    #[cfg(feature = "plus")]
    crate::model::organization::plan::PlanKind::new_for_project(
        conn_lock!(context),
        context.biller.as_ref(),
        &context.licensor,
        &query_project,
    )
    .await?;

    let update_project = UpdateProject::from(json_project.clone());
    diesel::update(schema::project::table.filter(schema::project::id.eq(query_project.id)))
        .set(&update_project)
        .execute(conn_lock!(context))
        .map_err(resource_conflict_err!(
            Project,
            (&query_project, &json_project)
        ))?;

    let new_query_project = QueryProject::get(conn_lock!(context), query_project.id)
        .map_err(resource_not_found_err!(Project, query_project))?;

    #[cfg(feature = "plus")]
    if query_project.slug == new_query_project.slug {
        context.update_index(log, &new_query_project).await;
    } else {
        context.delete_index(log, &query_project).await;
        context.update_index(log, &new_query_project).await;
    }

    new_query_project.into_json(conn_lock!(context))
}

/// Delete a project
///
/// Delete a project.
/// The user must have `delete` permissions for the project.
#[endpoint {
    method = DELETE,
    path =  "/v0/projects/{project}",
    tags = [ "projects"]
}]
pub async fn project_delete(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<ProjectParams>,
) -> Result<ResponseDeleted, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    delete_inner(
        &rqctx.log,
        rqctx.context(),
        path_params.into_inner(),
        &auth_user,
    )
    .await?;
    Ok(Delete::auth_response_deleted())
}

async fn delete_inner(
    log: &Logger,
    context: &ApiContext,
    path_params: ProjectParams,
    auth_user: &AuthUser,
) -> Result<(), HttpError> {
    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed(
        conn_lock!(context),
        &context.rbac,
        &path_params.project,
        auth_user,
        bencher_rbac::project::Permission::Delete,
    )?;

    diesel::delete(schema::project::table.filter(schema::project::id.eq(query_project.id)))
        .execute(conn_lock!(context))
        .map_err(resource_conflict_err!(Project, query_project))?;

    #[cfg(feature = "plus")]
    context.delete_index(log, &query_project).await;

    Ok(())
}
