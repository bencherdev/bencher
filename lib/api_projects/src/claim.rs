use bencher_endpoint::{CorsResponse, Endpoint, Post, ResponseCreated};
use bencher_json::{organization::member::OrganizationRole, JsonNewClaim, JsonProject, ResourceId};
use bencher_schema::{
    conn_lock,
    context::ApiContext,
    error::{issue_error, unauthorized_error},
    model::{
        organization::QueryOrganization,
        project::QueryProject,
        user::auth::{AuthUser, BearerToken},
    },
};
use dropshot::{endpoint, HttpError, Path, RequestContext, TypedBody};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, JsonSchema)]
pub struct ProjClaimParams {
    /// The slug or UUID for a project.
    pub project: ResourceId,
}

#[allow(clippy::no_effect_underscore_binding, clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/claim",
    tags = ["projects"]
}]
pub async fn proj_claim_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProjClaimParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Post.into()]))
}

/// Claim a project
///
/// Claim a project.
/// The user must be authenticated and the project must be unclaimed.
#[endpoint {
    method = POST,
    path =  "/v0/projects/{project}/claim",
    tags = ["projects"]
}]
pub async fn proj_claim_post(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<ProjClaimParams>,
    body: TypedBody<JsonNewClaim>,
) -> Result<ResponseCreated<JsonProject>, HttpError> {
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
    path_params: ProjClaimParams,
    _json_claim: JsonNewClaim,
    auth_user: AuthUser,
) -> Result<JsonProject, HttpError> {
    let query_project = QueryProject::from_resource_id(conn_lock!(context), &path_params.project)?;
    let query_organization =
        QueryOrganization::get(conn_lock!(context), query_project.organization_id)?;
    if query_organization.is_claimed(conn_lock!(context))? {
        return Err(unauthorized_error(format!(
            "This project ({}) has already been claimed.",
            path_params.project
        )));
    }

    // Create an invite token to claim the organization
    let invite = context
        .token_key
        .new_invite(
            auth_user.email.clone(),
            60,
            query_organization.uuid,
            OrganizationRole::Leader,
        )
        .map_err(|e| {
            issue_error(
                "Failed to create new claim token",
                "Failed to create new claim token.",
                e,
            )
        })?;
    auth_user
        .user
        .accept_invite(conn_lock!(context), &context.token_key, &invite)?;

    Ok(query_project.into_json_for_organization(conn_lock!(context), &query_organization))
}
