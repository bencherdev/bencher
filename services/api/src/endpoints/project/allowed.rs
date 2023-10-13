use bencher_json::{project::ProjectPermission, JsonAllowed, ResourceId};
use dropshot::{endpoint, HttpError, Path, RequestContext};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    context::ApiContext,
    endpoints::{
        endpoint::{response_ok, CorsResponse, ResponseOk},
        Endpoint,
    },
    model::{project::QueryProject, user::auth::AuthUser},
    ApiError,
};

#[derive(Deserialize, JsonSchema)]
pub struct ProjAllowedParams {
    pub project: ResourceId,
    pub permission: ProjectPermission,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/allowed/{permission}",
    tags = ["projects", "allowed"]
}]
pub async fn proj_allowed_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjAllowedParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Endpoint::GetOne]))
}

#[endpoint {
    method = GET,
    path = "/v0/projects/{project}/allowed/{permission}",
    tags = ["projects", "allowed"]
}]
pub async fn proj_allowed_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProjAllowedParams>,
) -> Result<ResponseOk<JsonAllowed>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::GetOne;

    let json = get_inner(rqctx.context(), path_params.into_inner(), &auth_user)
        .await
        .map_err(|e| {
            if let ApiError::HttpError(e) = e {
                e
            } else {
                endpoint.err(e).into()
            }
        })?;

    response_ok!(endpoint, json)
}

async fn get_inner(
    context: &ApiContext,
    path_params: ProjAllowedParams,
    auth_user: &AuthUser,
) -> Result<JsonAllowed, ApiError> {
    let conn = &mut *context.conn().await;

    Ok(JsonAllowed {
        allowed: QueryProject::is_allowed(
            conn,
            &context.rbac,
            &path_params.project,
            auth_user,
            crate::model::project::project_role::Permission::from(path_params.permission).into(),
        )
        .is_ok(),
    })
}
