use bencher_endpoint::{CorsResponse, Endpoint, Get, ResponseOk};
use bencher_json::{JsonAllowed, ProjectResourceId, project::ProjectPermission};
use bencher_schema::{
    conn_lock,
    context::ApiContext,
    model::{
        project::{QueryProject, project_role::Permission},
        user::auth::{AuthUser, BearerToken},
    },
};
use dropshot::{HttpError, Path, RequestContext, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, JsonSchema)]
pub struct ProjAllowedParams {
    /// The slug or UUID for a project.
    pub project: ProjectResourceId,
    /// The permission to check.
    pub permission: ProjectPermission,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/allowed/{permission}",
    tags = ["projects", "allowed"]
}]
pub async fn proj_allowed_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjAllowedParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

#[endpoint {
    method = GET,
    path = "/v0/projects/{project}/allowed/{permission}",
    tags = ["projects", "allowed"]
}]
pub async fn proj_allowed_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<ProjAllowedParams>,
) -> Result<ResponseOk<JsonAllowed>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = get_inner(rqctx.context(), path_params.into_inner(), &auth_user).await?;
    Ok(Get::auth_response_ok(json))
}

async fn get_inner(
    context: &ApiContext,
    path_params: ProjAllowedParams,
    auth_user: &AuthUser,
) -> Result<JsonAllowed, HttpError> {
    Ok(JsonAllowed {
        allowed: QueryProject::is_allowed(
            conn_lock!(context),
            &context.rbac,
            &path_params.project,
            auth_user,
            Permission::from(path_params.permission).into(),
        )
        .is_ok(),
    })
}
