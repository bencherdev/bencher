use std::sync::Arc;

use bencher_json::{organization::JsonOrganizationPermission, JsonAllowed, ResourceId};
use dropshot::{endpoint, HttpError, Path, RequestContext};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    context::Context,
    endpoints::{
        endpoint::{response_ok, ResponseOk},
        Endpoint, Method,
    },
    model::{organization::QueryOrganization, user::auth::AuthUser},
    util::cors::{get_cors, CorsResponse},
    ApiError,
};

use super::Resource;

const PERMISSION_RESOURCE: Resource = Resource::OrganizationPermission;

#[derive(Deserialize, JsonSchema)]
pub struct GetParams {
    pub organization: ResourceId,
    pub permission: JsonOrganizationPermission,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/organizations/{organization}/allowed/{permission}",
    tags = ["organizations", "allowed"]
}]
pub async fn options(
    _rqctx: Arc<RequestContext<Context>>,
    _path_params: Path<GetParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path = "/v0/organizations/{organization}/allowed/{permission}",
    tags = ["organizations", "allowed"]
}]
pub async fn get(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<GetParams>,
) -> Result<ResponseOk<JsonAllowed>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(PERMISSION_RESOURCE, Method::GetOne);

    let json = get_inner(rqctx.context(), path_params.into_inner(), &auth_user)
        .await
        .map_err(|e| endpoint.err(e))?;

    response_ok!(endpoint, json)
}

async fn get_inner(
    context: &Context,
    path_params: GetParams,
    auth_user: &AuthUser,
) -> Result<JsonAllowed, ApiError> {
    let api_context = &mut *context.lock().await;

    Ok(JsonAllowed {
        allowed: QueryOrganization::is_allowed_resource_id(
            api_context,
            &path_params.organization,
            auth_user,
            crate::model::organization::organization_role::Permission::from(path_params.permission)
                .into(),
        )
        .is_ok(),
    })
}
