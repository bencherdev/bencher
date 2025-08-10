use bencher_endpoint::{CorsResponse, Endpoint, Get, ResponseOk};
use bencher_json::{JsonAllowed, OrganizationResourceId, organization::OrganizationPermission};
use bencher_schema::{
    conn_lock,
    context::ApiContext,
    model::{
        organization::{QueryOrganization, organization_role::Permission},
        user::auth::{AuthUser, BearerToken},
    },
};
use dropshot::{HttpError, Path, RequestContext, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, JsonSchema)]
pub struct OrgAllowedParams {
    /// The slug or UUID for an organization.
    pub organization: OrganizationResourceId,
    /// The permission to check.
    pub permission: OrganizationPermission,
}

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
    bearer_token: BearerToken,
    path_params: Path<OrgAllowedParams>,
) -> Result<ResponseOk<JsonAllowed>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let json = get_inner(rqctx.context(), path_params.into_inner(), &auth_user).await?;
    Ok(Get::auth_response_ok(json))
}

async fn get_inner(
    context: &ApiContext,
    path_params: OrgAllowedParams,
    auth_user: &AuthUser,
) -> Result<JsonAllowed, HttpError> {
    Ok(JsonAllowed {
        allowed: QueryOrganization::is_allowed_resource_id(
            conn_lock!(context),
            &context.rbac,
            &path_params.organization,
            auth_user,
            Permission::from(path_params.permission).into(),
        )
        .is_ok(),
    })
}
