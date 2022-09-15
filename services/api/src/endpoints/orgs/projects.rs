use std::sync::Arc;

use bencher_json::{JsonNewProject, JsonProject, ResourceId};
use bencher_rbac::{
    organization::Permission as OrganizationPermission,
    project::{Permission as ProjectPermission, Role},
};
use diesel::{BoolExpressionMethods, ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::{endpoint, HttpError, Path, RequestContext, TypedBody};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    endpoints::{
        endpoint::{response_accepted, response_ok, ResponseAccepted, ResponseOk},
        Endpoint, Method,
    },
    error::api_error,
    model::{
        project::{InsertProject, QueryProject},
        user::{auth::AuthUser, project::InsertProjectRole},
    },
    schema,
    util::{
        cors::{get_cors, CorsResponse},
        resource_id::fn_resource_id,
        Context,
    },
    ApiError,
};

use super::Resource;

const PROJECT_RESOURCE: Resource = Resource::Project;

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects",
    tags = ["projects"]
}]
pub async fn dir_options(_rqctx: Arc<RequestContext<Context>>) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path = "/v0/projects",
    tags = ["projects"]
}]
pub async fn get_ls(
    rqctx: Arc<RequestContext<Context>>,
) -> Result<ResponseOk<Vec<JsonProject>>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(PROJECT_RESOURCE, Method::GetLs);

    let json = get_ls_inner(&auth_user, rqctx.context())
        .await
        .map_err(|e| endpoint.err(e))?;

    response_ok!(endpoint, json)
}

async fn get_ls_inner(
    auth_user: &AuthUser,
    context: &Context,
) -> Result<Vec<JsonProject>, ApiError> {
    let context = &mut *context.lock().await;
    let conn = &mut context.db_conn;

    let mut sql = schema::project::table.into_boxed();

    if !auth_user.is_admin(&context.rbac) {
        let organization = auth_user.organizations(&context.rbac, OrganizationPermission::View);
        // This is actually redundant for view permissions
        let projects = auth_user.projects(&context.rbac, ProjectPermission::View);
        sql = sql.filter(
            schema::project::organization_id
                .eq_any(organization)
                .or(schema::project::id.eq_any(projects)),
        );
    }

    Ok(sql
        .order(schema::project::name)
        .load::<QueryProject>(conn)
        .map_err(api_error!())?
        .into_iter()
        .filter_map(|query| query.into_json(conn).ok())
        .collect())
}

#[endpoint {
    method = POST,
    path = "/v0/projects",
    tags = ["projects"]
}]
pub async fn post(
    rqctx: Arc<RequestContext<Context>>,
    body: TypedBody<JsonNewProject>,
) -> Result<ResponseAccepted<JsonProject>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(PROJECT_RESOURCE, Method::Post);

    let json = post_inner(&auth_user, rqctx.context(), body.into_inner())
        .await
        .map_err(|e| endpoint.err(e))?;

    response_accepted!(endpoint, json)
}

async fn post_inner(
    auth_user: &AuthUser,
    context: &Context,
    json_project: JsonNewProject,
) -> Result<JsonProject, ApiError> {
    let context = &mut *context.lock().await;
    let conn = &mut context.db_conn;

    // Create the project
    let insert_project = InsertProject::from_json(conn, json_project)?;

    // Check to see if user has permission to create a project within the organization
    context.rbac.is_allowed_organization(
        auth_user,
        OrganizationPermission::Create,
        &insert_project,
    )?;

    diesel::insert_into(schema::project::table)
        .values(&insert_project)
        .execute(conn)
        .map_err(api_error!())?;
    let query_project = schema::project::table
        .filter(schema::project::uuid.eq(&insert_project.uuid))
        .first::<QueryProject>(conn)
        .map_err(api_error!())?;

    // Connect the user to the project as a `Maintainer`
    let insert_proj_role = InsertProjectRole {
        user_id: auth_user.id,
        project_id: query_project.id,
        role: Role::Maintainer.to_string(),
    };
    diesel::insert_into(schema::project_role::table)
        .values(&insert_proj_role)
        .execute(conn)
        .map_err(api_error!())?;

    query_project.into_json(conn)
}

#[derive(Deserialize, JsonSchema)]
pub struct GetOneParams {
    pub project: ResourceId,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}",
    tags = ["projects"]
}]
pub async fn one_options(
    _rqctx: Arc<RequestContext<Context>>,
    _path_params: Path<GetOneParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

fn_resource_id!(project);

#[endpoint {
    method = GET,
    path = "/v0/projects/{project}",
    tags = ["projects"]
}]
pub async fn get_one(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<GetOneParams>,
) -> Result<ResponseOk<JsonProject>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(PROJECT_RESOURCE, Method::GetOne);

    let json = get_one_inner(&auth_user, rqctx.context(), path_params.into_inner())
        .await
        .map_err(|e| endpoint.err(e))?;

    response_ok!(endpoint, json)
}

async fn get_one_inner(
    auth_user: &AuthUser,
    context: &Context,
    path_params: GetOneParams,
) -> Result<JsonProject, ApiError> {
    let context = &mut *context.lock().await;

    QueryProject::is_allowed(
        context,
        &path_params.project,
        auth_user,
        OrganizationPermission::View,
        ProjectPermission::View,
    )?
    .into_json(&mut context.db_conn)
}
