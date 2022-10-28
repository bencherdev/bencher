use std::sync::Arc;

use bencher_json::{JsonBranch, JsonNewBranch, ResourceId};
use bencher_rbac::project::Permission;
use diesel::{expression_methods::BoolExpressionMethods, ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::{endpoint, HttpError, Path, RequestContext, TypedBody};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    endpoints::{
        endpoint::{response_accepted, response_ok, ResponseAccepted, ResponseOk},
        Endpoint, Method,
    },
    error::api_error,
    model::project::{
        branch::{InsertBranch, QueryBranch},
        project::QueryProject,
    },
    model::user::auth::AuthUser,
    schema,
    util::{
        cors::{get_cors, CorsResponse},
        error::into_json,
        resource_id::fn_resource_id,
        Context,
    },
    ApiError,
};

use super::Resource;

const BRANCH_RESOURCE: Resource = Resource::Branch;

#[derive(Deserialize, JsonSchema)]
pub struct GetDirParams {
    pub project: ResourceId,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/branches",
    tags = ["projects", "branches"]
}]
pub async fn dir_options(
    _rqctx: Arc<RequestContext<Context>>,
    _path_params: Path<GetDirParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/branches",
    tags = ["projects", "branches"]
}]
pub async fn get_ls(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<GetDirParams>,
) -> Result<ResponseOk<Vec<JsonBranch>>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(BRANCH_RESOURCE, Method::GetLs);

    let json = get_ls_inner(
        rqctx.context(),
        &auth_user,
        path_params.into_inner(),
        endpoint,
    )
    .await
    .map_err(|e| endpoint.err(e))?;

    response_ok!(endpoint, json)
}

async fn get_ls_inner(
    context: &Context,
    auth_user: &AuthUser,
    path_params: GetDirParams,
    endpoint: Endpoint,
) -> Result<Vec<JsonBranch>, ApiError> {
    let api_context = &mut *context.lock().await;
    let query_project = QueryProject::is_allowed_resource_id(
        api_context,
        &path_params.project,
        auth_user,
        Permission::View,
    )?;
    let conn = &mut api_context.database;

    Ok(schema::branch::table
        .filter(schema::branch::project_id.eq(&query_project.id))
        .order(schema::branch::name)
        .load::<QueryBranch>(conn)
        .map_err(api_error!())?
        .into_iter()
        .filter_map(into_json!(endpoint, conn))
        .collect())
}

#[endpoint {
    method = POST,
    path =  "/v0/projects/{project}/branches",
    tags = ["projects", "branches"]
}]
pub async fn post(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<GetDirParams>,
    body: TypedBody<JsonNewBranch>,
) -> Result<ResponseAccepted<JsonBranch>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(BRANCH_RESOURCE, Method::Post);

    let json = post_inner(
        rqctx.context(),
        path_params.into_inner(),
        body.into_inner(),
        &auth_user,
    )
    .await
    .map_err(|e| endpoint.err(e))?;

    response_accepted!(endpoint, json)
}

async fn post_inner(
    context: &Context,
    path_params: GetDirParams,
    json_branch: JsonNewBranch,
    auth_user: &AuthUser,
) -> Result<JsonBranch, ApiError> {
    let api_context = &mut *context.lock().await;
    let insert_branch =
        InsertBranch::from_json(&mut api_context.database, &path_params.project, json_branch)?;
    // Verify that the user is allowed
    QueryProject::is_allowed_id(
        api_context,
        insert_branch.project_id,
        auth_user,
        Permission::Create,
    )?;
    let conn = &mut api_context.database;

    diesel::insert_into(schema::branch::table)
        .values(&insert_branch)
        .execute(conn)
        .map_err(api_error!())?;

    schema::branch::table
        .filter(schema::branch::uuid.eq(&insert_branch.uuid))
        .first::<QueryBranch>(conn)
        .map_err(api_error!())?
        .into_json(conn)
}

#[derive(Deserialize, JsonSchema)]
pub struct GetOneParams {
    pub project: ResourceId,
    pub branch: ResourceId,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/branches/{branch}",
    tags = ["projects", "branches"]
}]
pub async fn one_options(
    _rqctx: Arc<RequestContext<Context>>,
    _path_params: Path<GetOneParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/branches/{branch}",
    tags = ["projects", "branches"]
}]
pub async fn get_one(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<GetOneParams>,
) -> Result<ResponseOk<JsonBranch>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(BRANCH_RESOURCE, Method::GetOne);

    let json = get_one_inner(rqctx.context(), path_params.into_inner(), &auth_user)
        .await
        .map_err(|e| endpoint.err(e))?;

    response_ok!(endpoint, json)
}

fn_resource_id!(branch);

async fn get_one_inner(
    context: &Context,
    path_params: GetOneParams,
    auth_user: &AuthUser,
) -> Result<JsonBranch, ApiError> {
    let api_context = &mut *context.lock().await;
    let query_project = QueryProject::is_allowed_resource_id(
        api_context,
        &path_params.project,
        auth_user,
        Permission::View,
    )?;
    let conn = &mut api_context.database;

    schema::branch::table
        .filter(
            schema::branch::project_id
                .eq(query_project.id)
                .and(resource_id(&path_params.branch)?),
        )
        .first::<QueryBranch>(conn)
        .map_err(api_error!())?
        .into_json(conn)
}
