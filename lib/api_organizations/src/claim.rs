use bencher_endpoint::{CorsResponse, Endpoint, Post, ResponseCreated};
use bencher_json::{JsonNewClaim, JsonOrganization, OrganizationResourceId};
use bencher_schema::{
    auth_conn,
    context::ApiContext,
    model::{
        organization::QueryOrganization,
        user::auth::{AuthUser, BearerToken},
    },
};
use dropshot::{HttpError, Path, RequestContext, TypedBody, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, JsonSchema)]
pub struct OrgClaimParams {
    /// The slug or UUID for an organization.
    pub organization: OrganizationResourceId,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/organizations/{organization}/claim",
    tags = ["organizations"]
}]
pub async fn org_claim_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<OrgClaimParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Post.into()]))
}

/// Claim an organization
///
/// Claim an organization.
/// The user must be authenticated and the organization must be unclaimed.
#[endpoint {
    method = POST,
    path =  "/v0/organizations/{organization}/claim",
    tags = ["organizations"]
}]
pub async fn org_claim_post(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<OrgClaimParams>,
    body: TypedBody<JsonNewClaim>,
) -> Result<ResponseCreated<JsonOrganization>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    let log = &rqctx.log;
    let json = post_inner(
        log,
        rqctx.context(),
        path_params.into_inner(),
        body.into_inner(),
        auth_user,
    )
    .await?;
    Ok(Post::auth_response_created(json))
}

async fn post_inner(
    log: &slog::Logger,
    context: &ApiContext,
    path_params: OrgClaimParams,
    _json_claim: JsonNewClaim,
    auth_user: AuthUser,
) -> Result<JsonOrganization, HttpError> {
    let query_organization =
        QueryOrganization::from_resource_id(auth_conn!(context), &path_params.organization)?;
    query_organization
        .claim(log, context, &auth_user.user)
        .await?;

    Ok(query_organization.into_json(auth_conn!(context)))
}
