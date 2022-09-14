use std::sync::Arc;

use bencher_json::{JsonNewProject, JsonProject, ResourceId};
use bencher_rbac::{
    organization::Permission as OrganizationPermission,
    project::{Permission as ProjectPermission, Role},
};
use diesel::{BoolExpressionMethods, ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::{
    endpoint, HttpError, HttpResponseAccepted, HttpResponseHeaders, HttpResponseOk, Path,
    RequestContext, TypedBody,
};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    endpoints::{
        endpoint::{response_ok, ResponseOk},
        Endpoint, Method,
    },
    model::{
        project::{InsertProject, QueryProject},
        user::{auth::AuthUser, project::InsertProjectRole, QueryUser},
    },
    schema,
    util::{
        cors::{get_cors, CorsResponse},
        headers::CorsHeaders,
        map_http_error,
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

    let context = rqctx.context();
    let json = get_ls_inner(&auth_user, context)
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
    let organization = auth_user.organizations(&context.rbac, OrganizationPermission::View);
    // This is actually redundant for view permissions
    let projects = auth_user.projects(&context.rbac, ProjectPermission::View);

    Ok(schema::project::table
        .filter(
            schema::project::organization_id
                .eq_any(organization)
                .or(schema::project::id.eq_any(projects)),
        )
        .order(schema::project::name)
        .load::<QueryProject>(conn)
        .map_err(map_http_error!("Failed to get projects."))?
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
) -> Result<HttpResponseHeaders<HttpResponseAccepted<JsonProject>, CorsHeaders>, HttpError> {
    let user_id = QueryUser::auth(&rqctx).await?;

    let json_project = body.into_inner();

    let context = &mut *rqctx.context().lock().await;
    let conn = &mut context.db_conn;

    // Create the project
    let insert_project = InsertProject::from_json(conn, json_project)?;
    diesel::insert_into(schema::project::table)
        .values(&insert_project)
        .execute(conn)
        .map_err(map_http_error!("Failed to create project."))?;
    let query_project = schema::project::table
        .filter(schema::project::uuid.eq(&insert_project.uuid))
        .first::<QueryProject>(conn)
        .map_err(map_http_error!("Failed to create project."))?;

    // Connect the user to the project as a `Maintainer`
    let insert_proj_role = InsertProjectRole {
        user_id,
        project_id: query_project.id,
        role: Role::Maintainer.to_string(),
    };
    diesel::insert_into(schema::project_role::table)
        .values(&insert_proj_role)
        .execute(conn)
        .map_err(map_http_error!("Failed to create project."))?;

    let json = query_project.into_json(conn)?;

    Ok(HttpResponseHeaders::new(
        HttpResponseAccepted(json),
        CorsHeaders::new_auth("POST".into()),
    ))
}

#[derive(Deserialize, JsonSchema)]
pub struct PathParams {
    pub project: ResourceId,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}",
    tags = ["projects"]
}]
pub async fn one_options(
    _rqctx: Arc<RequestContext<Context>>,
    _path_params: Path<PathParams>,
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
    path_params: Path<PathParams>,
) -> Result<HttpResponseHeaders<HttpResponseOk<JsonProject>, CorsHeaders>, HttpError> {
    let user_id = QueryUser::auth(&rqctx).await?;
    let path_params = path_params.into_inner();

    let context = &mut *rqctx.context().lock().await;
    let conn = &mut context.db_conn;

    let project = path_params.project;
    let query = schema::project::table
        .filter(resource_id(&project)?)
        .first::<QueryProject>(conn)
        .map_err(map_http_error!("Failed to get project."))?;

    QueryUser::has_access(conn, user_id, query.id)?;
    let json = query.into_json(conn)?;

    Ok(HttpResponseHeaders::new(
        HttpResponseOk(json),
        CorsHeaders::new_pub("GET".into()),
    ))
}
