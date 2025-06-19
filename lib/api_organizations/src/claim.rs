use bencher_endpoint::{CorsResponse, Endpoint, Post, ResponseCreated};
use bencher_json::{JsonNewClaim, JsonOrganization, ResourceId};
use bencher_schema::{
    conn_lock,
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
    pub organization: ResourceId,
}

#[allow(clippy::no_effect_underscore_binding, clippy::unused_async)]
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
    let json = post_inner(
        rqctx.context(),
        path_params.into_inner(),
        body.into_inner(),
        auth_user,
    )
    .await?;
    Ok(Post::auth_response_created(json))
}

async fn post_inner(
    context: &ApiContext,
    path_params: OrgClaimParams,
    _json_claim: JsonNewClaim,
    auth_user: AuthUser,
) -> Result<JsonOrganization, HttpError> {
    let query_organization =
        QueryOrganization::from_resource_id(conn_lock!(context), &path_params.organization)?;
    query_organization.claim(context, &auth_user.user).await?;

    Ok(query_organization.into_json(conn_lock!(context)))
}
