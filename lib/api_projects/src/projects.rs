use bencher_endpoint::{CorsResponse, Delete, Endpoint, Get, Patch, ResponseDeleted, ResponseOk};
use bencher_json::{JsonProject, ProjectResourceId, Sanitize as _, project::JsonUpdateProject};
use bencher_rbac::project::Permission;
#[cfg(feature = "plus")]
use bencher_schema::model::organization::plan::PlanKind;
use bencher_schema::{
    actor_conn, auth_conn,
    context::ApiContext,
    error::{
        forbidden_error, resource_conflict_err, resource_not_found_err, with_auth_hint,
        with_token_hint,
    },
    model::{
        project::{QueryProject, UpdateProject},
        user::{
            actor::{ApiActor, PubProjectBearerToken},
            auth::{AuthUser, BearerToken},
        },
    },
    schema, write_conn,
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::{HttpError, Path, Query, RequestContext, TypedBody, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;
use slog::Logger;

#[derive(Deserialize, JsonSchema)]
pub struct ProjectParams {
    /// The slug or UUID for a project.
    pub project: ProjectResourceId,
}

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
/// A valid project key for the project may also be used for authentication.
#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}",
    tags = [ "projects"]
}]
pub async fn project_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubProjectBearerToken,
    path_params: Path<ProjectParams>,
) -> Result<ResponseOk<JsonProject>, HttpError> {
    let api_actor = ApiActor::from_token(
        &rqctx.log,
        rqctx.context(),
        #[cfg(feature = "plus")]
        rqctx.request.headers(),
        bearer_token,
    )
    .await?;
    let json = get_one_inner(rqctx.context(), path_params.into_inner(), &api_actor)
        .await
        .map_err(with_auth_hint)?;
    Ok(Get::response_ok(json, api_actor.is_auth()))
}

pub async fn get_one_inner(
    context: &ApiContext,
    path_params: ProjectParams,
    api_actor: &ApiActor,
) -> Result<JsonProject, HttpError> {
    actor_conn!(context, api_actor, |conn| {
        QueryProject::is_allowed_actor_pub(
            conn,
            &context.rbac,
            #[cfg(feature = "plus")]
            &context.rate_limiting,
            &path_params.project,
            api_actor,
        )?
        .into_json(conn)
    })
}

/// Update a project
///
/// Update a project.
/// The user must have `edit` permissions for the project.
/// Setting the `bmf_version` field requires a server admin.
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
        #[cfg(feature = "plus")]
        &rqctx.log,
        context,
        path_params.into_inner(),
        body.into_inner(),
        auth_user,
    )
    .await
    .map_err(with_token_hint)?;
    Ok(Patch::auth_response_ok(json))
}

async fn patch_inner(
    #[cfg(feature = "plus")] log: &Logger,
    context: &ApiContext,
    path_params: ProjectParams,
    json_project: JsonUpdateProject,
    auth_user: AuthUser,
) -> Result<JsonProject, HttpError> {
    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed(
        auth_conn!(context),
        &context.rbac,
        #[cfg(feature = "plus")]
        &context.rate_limiting,
        &path_params.project,
        &auth_user,
        Permission::Edit,
    )?;

    // Only a server admin can move the project's BMF version.
    if json_project.bmf_version().is_some() && !auth_user.is_admin(&context.rbac) {
        let mut auth_user = auth_user.clone();
        auth_user.sanitize();
        return Err(forbidden_error(format!(
            "Only admins can update the `bmf_version` field for a project. User is not an admin: {auth_user:?}",
        )));
    }

    QueryProject::is_web_url(json_project.url())?;

    // Check project visibility
    if let Some(visibility) = json_project.visibility() {
        #[cfg(not(feature = "plus"))]
        QueryProject::is_visibility_public(visibility)?;
        #[cfg(feature = "plus")]
        PlanKind::check_for_project(
            context,
            context.biller.as_ref(),
            &context.licensor,
            auth_user,
            &query_project,
            visibility,
        )
        .await?;
    }

    let update_project = UpdateProject::from(json_project.clone());
    diesel::update(schema::project::table.filter(schema::project::id.eq(query_project.id)))
        .set(&update_project)
        .execute(write_conn!(context))
        .map_err(resource_conflict_err!(
            Project,
            (&query_project, &json_project)
        ))?;

    let new_query_project = QueryProject::get(auth_conn!(context), query_project.id)
        .map_err(resource_not_found_err!(Project, query_project))?;

    #[cfg(feature = "plus")]
    if query_project.slug == new_query_project.slug {
        context.update_index(log, &new_query_project).await;
    } else {
        context.delete_index(log, &query_project).await;
        context.update_index(log, &new_query_project).await;
    }

    new_query_project.into_json(auth_conn!(context))
}

/// Delete a project
///
/// Delete a project.
/// The user must have `manage` permissions for the project.
/// By default, projects are soft-deleted.
/// Set the `hard` query parameter to `true` to permanently delete the project (requires server admin).
#[endpoint {
    method = DELETE,
    path =  "/v0/projects/{project}",
    tags = [ "projects"]
}]
pub async fn project_delete(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<ProjectParams>,
    query_params: Query<ProjectDeleteQuery>,
) -> Result<ResponseDeleted, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    delete_inner(
        &rqctx.log,
        rqctx.context(),
        path_params.into_inner(),
        query_params.into_inner(),
        &auth_user,
    )
    .await
    .map_err(with_token_hint)?;
    Ok(Delete::auth_response_deleted())
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProjectDeleteQuery {
    /// Hard delete the project (requires server admin).
    pub hard: Option<bool>,
}

async fn delete_inner(
    log: &Logger,
    context: &ApiContext,
    path_params: ProjectParams,
    query_params: ProjectDeleteQuery,
    auth_user: &AuthUser,
) -> Result<(), HttpError> {
    if query_params.hard.unwrap_or_default() {
        // Hard delete requires server admin
        if !auth_user.is_admin(&context.rbac) {
            return Err(forbidden_error("Hard delete requires server admin"));
        }
        // Server admin check above is stricter than per-resource RBAC.
        // Unfiltered lookup is needed to find soft-deleted entities.
        let query_project = schema::project::table
            .filter(QueryProject::eq_resource_id(&path_params.project))
            .first::<QueryProject>(auth_conn!(context))
            .map_err(resource_not_found_err!(Project, &path_params.project))?;
        diesel::delete(schema::project::table.filter(schema::project::id.eq(query_project.id)))
            .execute(write_conn!(context))
            .map_err(resource_conflict_err!(Project, query_project))?;

        #[cfg(feature = "plus")]
        context.delete_index(log, &query_project).await;
    } else {
        // Verify that the user is allowed (filtered — excludes soft-deleted)
        let query_project = QueryProject::is_allowed(
            auth_conn!(context),
            &context.rbac,
            #[cfg(feature = "plus")]
            &context.rate_limiting,
            &path_params.project,
            auth_user,
            Permission::Manage,
        )?;

        // Soft delete: replace slug/name with valid deleted sentinels to free UNIQUE constraints
        let now = context.clock.now();
        query_project.soft_delete(context, log, now).await?;
    }

    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::ProjectDelete);

    Ok(())
}
