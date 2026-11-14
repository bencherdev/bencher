//! An organization no one has claimed, named for the console before the reader presses Claim.

use bencher_endpoint::{CorsResponse, Endpoint, Get, ResponseOk};
use bencher_json::{JsonOrganizationName, OrganizationUuid, SignInRefusal};
use bencher_schema::{
    context::ApiContext,
    error::{conflict_error, issue_error, not_found_error},
    model::organization::QueryOrganization,
    public_conn,
};
use dropshot::{HttpError, Path, RequestContext, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::sign_in::{admit, refused};

#[derive(Deserialize, JsonSchema)]
pub struct AuthClaimParams {
    /// The UUID of an organization no one has claimed.
    pub organization: OrganizationUuid,
}

#[endpoint {
    method = OPTIONS,
    path = "/v0/auth/claim/{organization}",
    unpublished = true,
}]
pub async fn auth_claim_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<AuthClaimParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

/// Read an organization to claim
///
/// The name of an organization that no one has claimed, which only such an organization tells.
#[endpoint {
    method = GET,
    path = "/v0/auth/claim/{organization}",
    unpublished = true,
}]
pub async fn auth_claim_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<AuthClaimParams>,
) -> Result<ResponseOk<JsonOrganizationName>, HttpError> {
    let context = rqctx.context();
    admit(&rqctx.log, context, &rqctx.request)?;
    let AuthClaimParams { organization } = path_params.into_inner();
    let conn = public_conn!(context);
    let query_organization = QueryOrganization::find(conn, organization)
        .map_err(|e| {
            issue_error(
                "Failed to read an organization",
                "Failed to read an organization to claim:",
                e,
            )
        })?
        .ok_or_else(|| {
            refused(
                not_found_error("This organization no longer exists"),
                SignInRefusal::UnknownOrganization,
            )
        })?;
    if query_organization.is_claimed(conn)? {
        return Err(conflict_error("This organization has already been claimed"));
    }
    let QueryOrganization { name, slug, .. } = query_organization;
    Ok(Get::pub_response_ok(JsonOrganizationName { name, slug }))
}
