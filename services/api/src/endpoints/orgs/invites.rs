use std::sync::Arc;

use bencher_json::{auth::JsonInvite, jwt::JsonWebToken, JsonEmpty};
use bencher_rbac::organization::Permission as OrganizationPermission;
use dropshot::{endpoint, HttpError, RequestContext, TypedBody};
use tracing::info;

use crate::{
    endpoints::{
        endpoint::{response_accepted, ResponseAccepted},
        Endpoint, Method,
    },
    error::api_error,
    model::{organization::QueryOrganization, user::auth::AuthUser},
    util::{
        cors::{get_cors, CorsResponse},
        Context,
    },
    ApiError,
};

use super::Resource;

const INVITE_RESOURCE: Resource = Resource::Invite;

#[endpoint {
    method = OPTIONS,
    path =  "/v0/invites",
    tags = ["invites"]
}]
pub async fn options(_rqctx: Arc<RequestContext<Context>>) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = POST,
    path = "/v0/invites",
    tags = ["invites"]
}]
pub async fn post(
    rqctx: Arc<RequestContext<Context>>,
    body: TypedBody<JsonInvite>,
) -> Result<ResponseAccepted<JsonEmpty>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(INVITE_RESOURCE, Method::Post);

    let json = post_inner(rqctx.context(), body.into_inner(), &auth_user)
        .await
        .map_err(|e| endpoint.err(e))?;

    response_accepted!(endpoint, json)
}

async fn post_inner(
    context: &Context,
    json_invite: JsonInvite,
    auth_user: &AuthUser,
) -> Result<JsonEmpty, ApiError> {
    let api_context = &mut *context.lock().await;

    // Check to see if user has permission to create a project within the organization
    api_context.rbac.is_allowed_organization(
        auth_user,
        OrganizationPermission::CreateRoleAssignments,
        QueryOrganization::into_rbac(&mut api_context.db_conn, json_invite.organization)?,
    )?;

    let token = JsonWebToken::new_invite(
        &api_context.secret_key,
        json_invite.email.clone(),
        json_invite.organization,
        json_invite.role,
    )
    .map_err(api_error!())?;

    // TODO log this as trace if SMTP is configured
    info!("Accept invite for \"{}\" with: {token}", json_invite.email);

    Ok(JsonEmpty::default())
}
