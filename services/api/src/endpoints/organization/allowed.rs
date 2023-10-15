use bencher_json::{organization::OrganizationPermission, JsonAllowed, ResourceId};
use dropshot::{endpoint, HttpError, Path, RequestContext};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    context::ApiContext,
    endpoints::{
        endpoint::{CorsResponse, Get, ResponseOk},
        Endpoint,
    },
    model::{organization::QueryOrganization, user::auth::AuthUser},
};

#[derive(Deserialize, JsonSchema)]
pub struct OrgAllowedParams {
    pub organization: ResourceId,
    pub permission: OrganizationPermission,
}

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/organizations/{organization}/allowed/{permission}",
    tags = ["organizations", "allowed"]
}]
pub async fn org_allowed_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<OrgAllowedParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

#[endpoint {
    method = GET,
    path = "/v0/organizations/{organization}/allowed/{permission}",
    tags = ["organizations", "allowed"]
}]
pub async fn org_allowed_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<OrgAllowedParams>,
) -> Result<ResponseOk<JsonAllowed>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let json = get_inner(rqctx.context(), path_params.into_inner(), &auth_user).await?;
    Ok(Get::auth_response_ok(json))
}

async fn get_inner(
    context: &ApiContext,
    path_params: OrgAllowedParams,
    auth_user: &AuthUser,
) -> Result<JsonAllowed, HttpError> {
    let conn = &mut *context.conn().await;

    Ok(JsonAllowed {
        allowed: QueryOrganization::is_allowed_resource_id(
            conn,
            &context.rbac,
            &path_params.organization,
            auth_user,
            crate::model::organization::organization_role::Permission::from(path_params.permission)
                .into(),
        )
        .is_ok(),
    })
}
