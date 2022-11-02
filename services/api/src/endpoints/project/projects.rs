use std::sync::Arc;

use bencher_json::{JsonProject, ResourceId};
use bencher_rbac::organization::Permission;
use dropshot::{endpoint, HttpError, Path, RequestContext};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    context::Context,
    endpoints::{
        endpoint::{response_ok, ResponseOk},
        Endpoint, Method,
    },
    model::{organization::QueryOrganization, project::QueryProject, user::auth::AuthUser},
    util::cors::{get_cors, CorsResponse},
    ApiError,
};

use super::Resource;

const PROJECT_RESOURCE: Resource = Resource::Project;

#[derive(Deserialize, JsonSchema)]
pub struct OnePath {
    pub project: ResourceId,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}",
    tags = ["projects"]
}]
pub async fn one_options(
    _rqctx: Arc<RequestContext<Context>>,
    _path_params: Path<OnePath>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}",
    tags = [ "projects"]
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

    let query_project =
        QueryProject::from_resource_id(&mut api_context.database, &path_params.project)?;

    QueryOrganization::is_allowed_id(
        api_context,
        query_project.organization_id,
        auth_user,
        Permission::View,
    )?;

    query_project.into_json(&mut api_context.database)
}
