use bencher_json::{organization::OrganizationPermission, JsonAllowed, ResourceId};
use dropshot::{endpoint, HttpError, Path, RequestContext};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    context::ApiContext,
    endpoints::{
        endpoint::{response_ok, CorsResponse, ResponseOk},
        Endpoint,
    },
    model::{organization::QueryOrganization, user::auth::AuthUser},
    ApiError,
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
    Ok(Endpoint::cors(&[Endpoint::GetOne]))
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
    path_params: OrgAllowedParams,
    auth_user: &AuthUser,
) -> Result<JsonAllowed, ApiError> {
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
