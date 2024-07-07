use bencher_json::{
    project::branch::JsonUpdateBranch, BranchName, JsonBranch, JsonBranches, JsonDirection,
    JsonNewBranch, JsonPagination, ResourceId,
};
use bencher_rbac::project::Permission;
use diesel::{
    BelongingToDsl, BoolExpressionMethods, ExpressionMethods, QueryDsl, RunQueryDsl,
    TextExpressionMethods,
};
use dropshot::{endpoint, HttpError, Path, Query, RequestContext, TypedBody};
use schemars::JsonSchema;
use serde::Deserialize;
use slog::Logger;

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
            branch::{QueryBranch, UpdateBranch},
            QueryProject,
        },
        user::auth::{AuthUser, BearerToken, PubBearerToken},
    },
    schema,
    util::{headers::TotalCount, search::Search},
};

#[derive(Deserialize, JsonSchema)]
pub struct ProjBranchesParams {
    /// The slug or UUID for a project.
    pub project: ResourceId,
}

pub type ProjBranchesPagination = JsonPagination<ProjBranchesSort>;

#[derive(Debug, Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProjBranchesSort {
    /// Sort by branch name.
    #[default]
    Name,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProjBranchesQuery {
    /// Filter by branch name, exact match.
    pub name: Option<BranchName>,
    /// Search by branch name, slug, or UUID.
    pub search: Option<Search>,
    /// Only return archived branches.
    pub archived: Option<bool>,
}

#[allow(clippy::no_effect_underscore_binding, clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/branches",
    tags = ["projects", "branches"]
}]
pub async fn proj_branches_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjBranchesParams>,
    _pagination_params: Query<ProjBranchesPagination>,
    _query_params: Query<ProjBranchesQuery>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Post.into()]))
}

/// List branches for a project
///
/// List all branches for a project.
/// If the project is public, then the user does not need to be authenticated.
/// If the project is private, then the user must be authenticated and have `view` permissions for the project.
/// By default, the branches are sorted in alphabetical order by name.
/// The HTTP response header `X-Total-Count` contains the total number of branches.
#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/branches",
    tags = ["projects", "branches"]
}]
pub async fn proj_branches_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjBranchesParams>,
    pagination_params: Query<ProjBranchesPagination>,
    query_params: Query<ProjBranchesQuery>,
) -> Result<ResponseOk<JsonBranches>, HttpError> {
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
    path_params: ProjBranchesParams,
    pagination_params: ProjBranchesPagination,
    query_params: ProjBranchesQuery,
) -> Result<(JsonBranches, TotalCount), HttpError> {
    let query_project = QueryProject::is_allowed_public(
        conn_lock!(context),
        &context.rbac,
        &path_params.project,
        auth_user,
    )?;

    let branches = get_ls_query(&query_project, &pagination_params, &query_params)
        .offset(pagination_params.offset())
        .limit(pagination_params.limit())
        .load::<QueryBranch>(conn_lock!(context))
        .map_err(resource_not_found_err!(
            Branch,
            (&query_project, &pagination_params, &query_params)
        ))?;

    // Separate out these queries to prevent a deadlock when getting the conn_lock
    let mut json_branches = Vec::with_capacity(branches.len());
    for branch in branches {
        match branch.into_json_for_project(conn_lock!(context), &query_project) {
            Ok(branch) => json_branches.push(branch),
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
            Branch,
            (&query_project, &pagination_params, &query_params)
        ))?
        .try_into()?;

    Ok((json_branches.into(), total_count))
}

fn get_ls_query<'q>(
    query_project: &'q QueryProject,
    pagination_params: &ProjBranchesPagination,
    query_params: &'q ProjBranchesQuery,
) -> schema::branch::BoxedQuery<'q, diesel::sqlite::Sqlite> {
    let mut query = QueryBranch::belonging_to(query_project).into_boxed();

    if let Some(name) = query_params.name.as_ref() {
        query = query.filter(schema::branch::name.eq(name));
    }
    if let Some(search) = query_params.search.as_ref() {
        query = query.filter(
            schema::branch::name
                .like(search)
                .or(schema::branch::slug.like(search))
                .or(schema::branch::uuid.like(search)),
        );
    }

    if let Some(true) = query_params.archived {
        query = query.filter(schema::branch::archived.is_not_null());
    } else {
        query = query.filter(schema::branch::archived.is_null());
    };

    match pagination_params.order() {
        ProjBranchesSort::Name => match pagination_params.direction {
            Some(JsonDirection::Asc) | None => query.order(schema::branch::name.asc()),
            Some(JsonDirection::Desc) => query.order(schema::branch::name.desc()),
        },
    }
}

/// Create a branch
///
/// Create a branch for a project.
/// The user must have `create` permissions for the project.
#[endpoint {
    method = POST,
    path =  "/v0/projects/{project}/branches",
    tags = ["projects", "branches"]
}]
pub async fn proj_branch_post(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<ProjBranchesParams>,
    body: TypedBody<JsonNewBranch>,
) -> Result<ResponseCreated<JsonBranch>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = post_inner(
        &rqctx.log,
        rqctx.context(),
        path_params.into_inner(),
        body.into_inner(),
        &auth_user,
    )
    .await?;
    Ok(Post::auth_response_created(json))
}

async fn post_inner(
    log: &Logger,
    context: &ApiContext,
    path_params: ProjBranchesParams,
    json_branch: JsonNewBranch,
    auth_user: &AuthUser,
) -> Result<JsonBranch, HttpError> {
    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed(
        conn_lock!(context),
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Create,
    )?;

    let query_branch =
        QueryBranch::create_from_json(log, context, query_project.id, json_branch).await?;

    query_branch.into_json_for_project(conn_lock!(context), &query_project)
}

#[derive(Deserialize, JsonSchema)]
pub struct ProjBranchParams {
    /// The slug or UUID for a project.
    pub project: ResourceId,
    /// The slug or UUID for a branch.
    pub branch: ResourceId,
}

#[allow(clippy::no_effect_underscore_binding, clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/branches/{branch}",
    tags = ["projects", "branches"]
}]
pub async fn proj_branch_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjBranchParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Patch.into(), Delete.into()]))
}

/// View a branch
///
/// View a branch for a project.
/// If the project is public, then the user does not need to be authenticated.
/// If the project is private, then the user must be authenticated and have `view` permissions for the project.
#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/branches/{branch}",
    tags = ["projects", "branches"]
}]
pub async fn proj_branch_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: PubBearerToken,
    path_params: Path<ProjBranchParams>,
) -> Result<ResponseOk<JsonBranch>, HttpError> {
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
    path_params: ProjBranchParams,
    auth_user: Option<&AuthUser>,
) -> Result<JsonBranch, HttpError> {
    let query_project = QueryProject::is_allowed_public(
        conn_lock!(context),
        &context.rbac,
        &path_params.project,
        auth_user,
    )?;

    conn_lock!(context, |conn| QueryBranch::belonging_to(&query_project)
        .filter(QueryBranch::eq_resource_id(&path_params.branch)?)
        .first::<QueryBranch>(conn)
        .map_err(resource_not_found_err!(
            Branch,
            (&query_project, path_params.branch)
        ))
        .and_then(
            |branch| branch.into_json_for_project(conn, &query_project)
        ))
}

/// Update a branch
///
/// Update a branch for a project.
/// The user must have `edit` permissions for the project.
#[endpoint {
    method = PATCH,
    path =  "/v0/projects/{project}/branches/{branch}",
    tags = ["projects", "branches"]
}]
pub async fn proj_branch_patch(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<ProjBranchParams>,
    body: TypedBody<JsonUpdateBranch>,
) -> Result<ResponseOk<JsonBranch>, HttpError> {
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
    path_params: ProjBranchParams,
    json_branch: JsonUpdateBranch,
    auth_user: &AuthUser,
) -> Result<JsonBranch, HttpError> {
    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed(
        conn_lock!(context),
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Edit,
    )?;

    let query_branch =
        QueryBranch::from_resource_id(conn_lock!(context), query_project.id, &path_params.branch)?;

    let update_branch = UpdateBranch::from(json_branch.clone());
    diesel::update(schema::branch::table.filter(schema::branch::id.eq(query_branch.id)))
        .set(&update_branch)
        .execute(conn_lock!(context))
        .map_err(resource_conflict_err!(
            Branch,
            (&query_branch, &json_branch)
        ))?;

    conn_lock!(context, |conn| QueryBranch::get(conn, query_branch.id)
        .map_err(resource_not_found_err!(Branch, query_branch))
        .and_then(
            |branch| branch.into_json_for_project(conn, &query_project)
        ))
}

/// Delete a branch
///
/// Delete a branch for a project.
/// The user must have `delete` permissions for the project.
/// All reports and thresholds that use this branch must be deleted first!
#[endpoint {
    method = DELETE,
    path =  "/v0/projects/{project}/branches/{branch}",
    tags = ["projects", "branches"]
}]
pub async fn proj_branch_delete(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<ProjBranchParams>,
) -> Result<ResponseDeleted, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    delete_inner(rqctx.context(), path_params.into_inner(), &auth_user).await?;
    Ok(Delete::auth_response_deleted())
}

async fn delete_inner(
    context: &ApiContext,
    path_params: ProjBranchParams,
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

    let query_branch =
        QueryBranch::from_resource_id(conn_lock!(context), query_project.id, &path_params.branch)?;

    diesel::delete(schema::branch::table.filter(schema::branch::id.eq(query_branch.id)))
        .execute(conn_lock!(context))
        .map_err(resource_conflict_err!(Branch, query_branch))?;

    Ok(())
}
