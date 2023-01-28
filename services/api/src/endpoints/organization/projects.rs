use std::sync::Arc;

use bencher_json::{JsonNewProject, JsonProject, ResourceId};
use bencher_rbac::{organization::Permission, project::Role};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::{endpoint, HttpError, Path, RequestContext, TypedBody};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    context::{ApiContext, Context},
    endpoints::{
        endpoint::{response_accepted, response_ok, ResponseAccepted, ResponseOk},
        Endpoint, Method,
    },
    error::api_error,
    model::{
        organization::QueryOrganization,
        project::{
            branch::InsertBranch, metric_kind::InsertMetricKind, project_role::InsertProjectRole,
            testbed::InsertTestbed, InsertProject, QueryProject,
        },
        user::auth::AuthUser,
    },
    schema,
    util::{
        cors::{get_cors, CorsResponse},
        error::into_json,
    },
    ApiError,
};

use super::Resource;

const PROJECT_RESOURCE: Resource = Resource::Project;

#[derive(Deserialize, JsonSchema)]
pub struct DirPath {
    pub organization: ResourceId,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/organizations/{organization}/projects",
    tags = ["organizations", "projects"]
}]
pub async fn dir_options(
    _rqctx: Arc<RequestContext<Context>>,
    _path_params: Path<DirPath>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path =  "/v0/organizations/{organization}/projects",
    tags = ["organizations", "projects"]
}]
pub async fn get_ls(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<DirPath>,
) -> Result<ResponseOk<Vec<JsonProject>>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(PROJECT_RESOURCE, Method::GetLs);

    let json = get_ls_inner(
        rqctx.context(),
        path_params.into_inner(),
        &auth_user,
        endpoint,
    )
    .await
    .map_err(|e| endpoint.err(e))?;

    response_ok!(endpoint, json)
}

async fn get_ls_inner(
    context: &Context,
    path_params: DirPath,
    auth_user: &AuthUser,
    endpoint: Endpoint,
) -> Result<Vec<JsonProject>, ApiError> {
    let api_context = &mut *context.lock().await;
    let query_organization = QueryOrganization::is_allowed_resource_id(
        api_context,
        &path_params.organization,
        auth_user,
        Permission::View,
    )?;
    let conn = &mut api_context.database.connection;

    Ok(schema::project::table
        .filter(schema::project::organization_id.eq(query_organization.id))
        .order((schema::project::name, schema::project::slug))
        .load::<QueryProject>(conn)
        .map_err(api_error!())?
        .into_iter()
        .filter_map(into_json!(endpoint, conn))
        .collect())
}

#[endpoint {
    method = POST,
    path =  "/v0/organizations/{organization}/projects",
    tags = ["organizations", "projects"]
}]
pub async fn post(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<DirPath>,
    body: TypedBody<JsonNewProject>,
) -> Result<ResponseAccepted<JsonProject>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(PROJECT_RESOURCE, Method::Post);

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
    path_params: DirPath,
    json_project: JsonNewProject,
    auth_user: &AuthUser,
) -> Result<JsonProject, ApiError> {
    let api_context = &mut *context.lock().await;
    // Check to see if it is a private project
    is_private_project(api_context, auth_user, json_project.public)?;

    let conn = &mut api_context.database.connection;

    // Create the project
    let insert_project = InsertProject::from_json(conn, &path_params.organization, json_project)?;

    // Check to see if user has permission to create a project within the organization
    api_context
        .rbac
        .is_allowed_organization(auth_user, Permission::Create, &insert_project)?;

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

    // Add a `main` branch to the project
    let insert_branch = InsertBranch::main(conn, query_project.id);
    diesel::insert_into(schema::branch::table)
        .values(&insert_branch)
        .execute(conn)
        .map_err(api_error!())?;

    // Add a `localhost` testbed to the project
    let insert_testbed = InsertTestbed::localhost(conn, query_project.id);
    diesel::insert_into(schema::testbed::table)
        .values(&insert_testbed)
        .execute(conn)
        .map_err(api_error!())?;

    // Add a `latency` metric kind to the project
    let insert_metric_kind = InsertMetricKind::latency(conn, query_project.id);
    diesel::insert_into(schema::metric_kind::table)
        .values(&insert_metric_kind)
        .execute(conn)
        .map_err(api_error!())?;

    query_project.into_json(conn)
}

// TODO private projects
fn is_private_project(
    api_context: &mut ApiContext,
    auth_user: &AuthUser,
    public: Option<bool>,
) -> Result<(), ApiError> {
    if let Some(false) = public {
        #[cfg(feature = "plus")]
        if bencher_plus::is_bencher_dev(&api_context.endpoint)
            && auth_user.is_admin(&api_context.rbac)
        {
            return Ok(());
        }

        Err(ApiError::CreatePrivateProject(auth_user.id))
    } else {
        Ok(())
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct OnePath {
    pub organization: ResourceId,
    pub project: ResourceId,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/organizations/{organization}/projects/{project}",
    tags = ["organizations", "projects"]
}]
pub async fn one_options(
    _rqctx: Arc<RequestContext<Context>>,
    _path_params: Path<OnePath>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path =  "/v0/organizations/{organization}/projects/{project}",
    tags = ["organizations", "projects"]
}]
pub async fn get_one(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<OnePath>,
) -> Result<ResponseOk<JsonProject>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(PROJECT_RESOURCE, Method::GetOne);

    let json = get_one_inner(rqctx.context(), path_params.into_inner(), &auth_user)
        .await
        .map_err(|e| endpoint.err(e))?;

    response_ok!(endpoint, json)
}

async fn get_one_inner(
    context: &Context,
    path_params: OnePath,
    auth_user: &AuthUser,
) -> Result<JsonProject, ApiError> {
    let api_context = &mut *context.lock().await;

    QueryOrganization::is_allowed_resource_id(
        api_context,
        &path_params.organization,
        auth_user,
        Permission::View,
    )?;

    let conn = &mut api_context.database.connection;
    QueryProject::from_resource_id(conn, &path_params.project)?.into_json(conn)
}
