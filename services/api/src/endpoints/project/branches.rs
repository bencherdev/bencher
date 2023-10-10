use bencher_json::{
    project::branch::JsonUpdateBranch, BranchName, JsonBranch, JsonBranches, JsonDirection,
    JsonEmpty, JsonNewBranch, JsonPagination, ResourceId,
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
        branch::{InsertBranch, QueryBranch, UpdateBranch},
        QueryProject,
    },
    model::user::auth::AuthUser,
    schema,
    util::{error::into_json, resource_id::fn_resource_id},
    ApiError,
};

#[derive(Deserialize, JsonSchema)]
pub struct ProjBranchesParams {
    pub project: ResourceId,
}

pub type ProjBranchesPagination = JsonPagination<ProjBranchesSort>;

#[derive(Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProjBranchesSort {
    #[default]
    Name,
}

#[derive(Deserialize, JsonSchema)]
pub struct ProjBranchesQuery {
    pub name: Option<BranchName>,
}

#[allow(clippy::unused_async)]
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
    Ok(Endpoint::cors(&[Endpoint::GetLs, Endpoint::Post]))
}

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
    path_params: ProjBranchesParams,
    pagination_params: ProjBranchesPagination,
    query_params: ProjBranchesQuery,
    endpoint: Endpoint,
) -> Result<JsonBranches, ApiError> {
    let conn = &mut *context.conn().await;

    let query_project =
        QueryProject::is_allowed_public(conn, &context.rbac, &path_params.project, auth_user)?;

    let mut query = QueryBranch::belonging_to(&query_project).into_boxed();

    if let Some(name) = query_params.name.as_ref() {
        query = query.filter(schema::branch::name.eq(name.as_ref()));
    }

    query = match pagination_params.order() {
        ProjBranchesSort::Name => match pagination_params.direction {
            Some(JsonDirection::Asc) | None => query.order(schema::branch::name.asc()),
            Some(JsonDirection::Desc) => query.order(schema::branch::name.desc()),
        },
    };

    Ok(query
        .offset(pagination_params.offset())
        .limit(pagination_params.limit())
        .load::<QueryBranch>(conn)
        .map_err(ApiError::from)?
        .into_iter()
        .filter_map(into_json!(endpoint, conn))
        .collect())
}

#[endpoint {
    method = POST,
    path =  "/v0/projects/{project}/branches",
    tags = ["projects", "branches"]
}]
pub async fn proj_branch_post(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjBranchesParams>,
    body: TypedBody<JsonNewBranch>,
) -> Result<ResponseAccepted<JsonBranch>, HttpError> {
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
    path_params: ProjBranchesParams,
    mut json_branch: JsonNewBranch,
    auth_user: &AuthUser,
) -> Result<JsonBranch, ApiError> {
    let conn = &mut *context.conn().await;

    // Verify that the user is allowed
    let query_project = QueryProject::is_allowed(
        conn,
        &context.rbac,
        &path_params.project,
        auth_user,
        Permission::Create,
    )?;

    // Soft creation
    // If the new branch name already exists then return the existing branch name
    // instead of erroring due to the unique constraint
    // This is useful to help prevent race conditions in CI
    if let Some(true) = json_branch.soft {
        if let Ok(branch) = QueryBranch::belonging_to(&query_project)
            .filter(schema::branch::name.eq(json_branch.name.as_ref()))
            .first::<QueryBranch>(conn)
        {
            return branch.into_json(conn);
        }
    }
    let start_point = json_branch.start_point.take();
    let insert_branch = InsertBranch::from_json(conn, query_project.id, json_branch);

    diesel::insert_into(schema::branch::table)
        .values(&insert_branch)
        .execute(conn)
        .map_err(ApiError::from)?;

    // Clone data and optionally thresholds from the start point
    if let Some(start_point) = &start_point {
        insert_branch.start_point(conn, start_point)?;
    }

    schema::branch::table
        .filter(schema::branch::uuid.eq(&insert_branch.uuid))
        .first::<QueryBranch>(conn)
        .map_err(ApiError::from)?
        .into_json(conn)
}

#[derive(Deserialize, JsonSchema)]
pub struct ProjBranchParams {
    pub project: ResourceId,
    pub branch: ResourceId,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/branches/{branch}",
    tags = ["projects", "branches"]
}]
pub async fn proj_branch_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjBranchParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[
        Endpoint::GetOne,
        Endpoint::Patch,
        Endpoint::Delete,
    ]))
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/branches/{branch}",
    tags = ["projects", "branches"]
}]
pub async fn proj_branch_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjBranchParams>,
) -> Result<ResponseOk<JsonBranch>, HttpError> {
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

fn_resource_id!(branch);

async fn get_one_inner(
    context: &ApiContext,
    path_params: ProjBranchParams,
    auth_user: Option<&AuthUser>,
) -> Result<JsonBranch, ApiError> {
    let conn = &mut *context.conn().await;

    let query_project =
        QueryProject::is_allowed_public(conn, &context.rbac, &path_params.project, auth_user)?;

    QueryBranch::belonging_to(&query_project)
        .filter(resource_id(&path_params.branch)?)
        .first::<QueryBranch>(conn)
        .map_err(ApiError::from)?
        .into_json(conn)
}

#[endpoint {
    method = PATCH,
    path =  "/v0/projects/{project}/branches/{branch}",
    tags = ["projects", "branches"]
}]
pub async fn proj_branch_patch(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjBranchParams>,
    body: TypedBody<JsonUpdateBranch>,
) -> Result<ResponseAccepted<JsonBranch>, HttpError> {
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
    path_params: ProjBranchParams,
    json_branch: JsonUpdateBranch,
    auth_user: &AuthUser,
) -> Result<JsonBranch, ApiError> {
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

    let query_branch = QueryBranch::from_resource_id(conn, project_id, &path_params.branch)?;
    if query_branch.is_system() {
        return Err(ApiError::SystemBranch);
    }
    diesel::update(schema::branch::table.filter(schema::branch::id.eq(query_branch.id)))
        .set(&UpdateBranch::from(json_branch))
        .execute(conn)
        .map_err(ApiError::from)?;

    QueryBranch::get(conn, query_branch.id)?.into_json(conn)
}

#[endpoint {
    method = DELETE,
    path =  "/v0/projects/{project}/branches/{branch}",
    tags = ["projects", "branches"]
}]
pub async fn proj_branch_delete(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjBranchParams>,
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
    path_params: ProjBranchParams,
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

    let query_branch = QueryBranch::from_resource_id(conn, project_id, &path_params.branch)?;
    if query_branch.is_system() {
        return Err(ApiError::SystemBranch);
    }
    diesel::delete(schema::branch::table.filter(schema::branch::id.eq(query_branch.id)))
        .execute(conn)
        .map_err(ApiError::from)?;

    Ok(JsonEmpty {})
}
