use bencher_endpoint::{
    CorsResponse, Delete, Endpoint, Get, Post, ResponseAccepted, ResponseDeleted, ResponseOk,
    TotalCount,
};
use bencher_json::{
    InviteSecret, InviteUuid, JsonDirection, JsonInvite, JsonInvites, JsonPagination, JsonPubUser,
    OrganizationResourceId, UserName, UserSlug, UserUuid,
};
use bencher_rbac::organization::Permission;
use bencher_schema::{
    auth_conn,
    context::{ApiContext, Body, ButtonBody, Message},
    error::{forbidden_error, issue_error, resource_not_found_err, service_unavailable_error},
    model::{
        organization::{
            QueryOrganization,
            invite::{INVITE_LIFETIME, QueryInvite},
        },
        user::{
            QueryUser,
            auth::{AuthUser, BearerToken},
        },
    },
    schema, write_conn,
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::{HttpError, Path, Query, RequestContext, endpoint};
use schemars::JsonSchema;
use serde::Deserialize;
use slog::Logger;

/// The console's page an invitation's link opens, with its secret in the fragment.
const INVITE_PATH: &str = "/auth/login";
const ONLY_A_SESSION: &str = "Only a user session can invite a member.";

#[derive(Deserialize, JsonSchema)]
pub struct OrgInvitesParams {
    /// The slug or UUID for an organization.
    pub organization: OrganizationResourceId,
}

pub type OrgInvitesPagination = JsonPagination<OrgInvitesSort>;

#[derive(Debug, Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OrgInvitesSort {
    /// Sort by when the invitation was sent, newest first unless the direction is `asc`.
    #[default]
    Created,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/organizations/{organization}/invites",
    tags = ["organizations", "invites"]
}]
pub async fn org_invites_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<OrgInvitesParams>,
    _pagination_params: Query<OrgInvitesPagination>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

/// List pending invitations
///
/// List the invitations to an organization that have not been accepted or revoked,
/// expired ones included.
/// The user must have `create_role` permissions for the organization.
/// By default, the newest invitations come first.
/// The HTTP response header `X-Total-Count` contains the total number of invitations.
#[endpoint {
    method = GET,
    path =  "/v0/organizations/{organization}/invites",
    tags = ["organizations", "invites"]
}]
pub async fn org_invites_get(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<OrgInvitesParams>,
    pagination_params: Query<OrgInvitesPagination>,
) -> Result<ResponseOk<JsonInvites>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let (json, total_count) = get_ls_inner(
        rqctx.context(),
        &auth_user,
        path_params.into_inner(),
        pagination_params.into_inner(),
    )
    .await?;
    Ok(Get::auth_response_ok_with_total_count(json, total_count))
}

async fn get_ls_inner(
    context: &ApiContext,
    auth_user: &AuthUser,
    path_params: OrgInvitesParams,
    pagination_params: OrgInvitesPagination,
) -> Result<(JsonInvites, TotalCount), HttpError> {
    let query_organization = QueryOrganization::is_allowed_resource_id(
        auth_conn!(context),
        &context.rbac,
        &path_params.organization,
        auth_user,
        Permission::CreateRole,
    )?;

    let query = schema::invite::table
        .inner_join(schema::user::table)
        .filter(schema::invite::organization_id.eq(query_organization.id));
    let ordered = match pagination_params.order() {
        OrgInvitesSort::Created => match pagination_params.direction {
            Some(JsonDirection::Asc) => query
                .order((schema::invite::created.asc(), schema::invite::id.asc()))
                .into_boxed(),
            Some(JsonDirection::Desc) | None => query
                .order((schema::invite::created.desc(), schema::invite::id.desc()))
                .into_boxed(),
        },
    };
    let invites = ordered
        .select((
            schema::invite::all_columns,
            (schema::user::uuid, schema::user::name, schema::user::slug),
        ))
        .offset(pagination_params.offset())
        .limit(pagination_params.limit())
        .load::<(QueryInvite, (UserUuid, UserName, UserSlug))>(auth_conn!(context))
        .map_err(resource_not_found_err!(
            Invite,
            (&query_organization, &pagination_params)
        ))?;

    // Drop connection lock before iterating
    let json_invites = invites
        .into_iter()
        .map(|(invite, (uuid, name, slug))| invite.into_json(JsonPubUser { uuid, name, slug }))
        .collect();

    let total_count = schema::invite::table
        .filter(schema::invite::organization_id.eq(query_organization.id))
        .count()
        .get_result::<i64>(auth_conn!(context))
        .map_err(resource_not_found_err!(
            Invite,
            (&query_organization, &pagination_params)
        ))?
        .try_into()?;

    Ok((json_invites, total_count))
}

#[derive(Deserialize, JsonSchema)]
pub struct OrgInviteParams {
    /// The slug or UUID for an organization.
    pub organization: OrganizationResourceId,
    /// The UUID for an invitation.
    pub invite: InviteUuid,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/organizations/{organization}/invites/{invite}",
    tags = ["organizations", "invites"]
}]
pub async fn org_invite_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<OrgInviteParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Delete.into()]))
}

/// Revoke an invitation
///
/// End a pending invitation, so its link no longer works.
/// The user must have `delete_role` permissions for the organization.
#[endpoint {
    method = DELETE,
    path =  "/v0/organizations/{organization}/invites/{invite}",
    tags = ["organizations", "invites"]
}]
pub async fn org_invite_delete(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<OrgInviteParams>,
) -> Result<ResponseDeleted, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    delete_inner(rqctx.context(), path_params.into_inner(), &auth_user).await?;
    Ok(Delete::auth_response_deleted())
}

async fn delete_inner(
    context: &ApiContext,
    path_params: OrgInviteParams,
    auth_user: &AuthUser,
) -> Result<(), HttpError> {
    let query_organization = QueryOrganization::is_allowed_resource_id(
        auth_conn!(context),
        &context.rbac,
        &path_params.organization,
        auth_user,
        Permission::DeleteRole,
    )?;
    let query_invite = QueryInvite::from_uuid(
        auth_conn!(context),
        query_organization.id,
        path_params.invite,
    )?;
    QueryInvite::end(write_conn!(context), query_invite.id).map_err(|e| {
        issue_error(
            "Failed to revoke an invitation",
            "Failed to revoke an invitation:",
            e,
        )
    })?;
    Ok(())
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/organizations/{organization}/invites/{invite}/resend",
    tags = ["organizations", "invites"]
}]
pub async fn org_invite_resend_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<OrgInviteParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Post.into()]))
}

/// Resend an invitation
///
/// Email a pending invitation again, expired or not, with a new link that works for 7 days.
/// The link it sent before no longer works.
/// The user must have `create_role` permissions for the organization.
/// This endpoint requires a user session signed in within the last 10 minutes:
/// neither a user API key nor a deprecated user API token can send an invitation.
#[endpoint {
    method = POST,
    path =  "/v0/organizations/{organization}/invites/{invite}/resend",
    tags = ["organizations", "invites"]
}]
pub async fn org_invite_resend_post(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    path_params: Path<OrgInviteParams>,
) -> Result<ResponseAccepted<JsonInvite>, HttpError> {
    let auth_user = AuthUser::from_token(rqctx.context(), bearer_token).await?;
    check_inviter(rqctx.context(), &auth_user).await?;
    let json = resend_inner(
        &rqctx.log,
        rqctx.context(),
        path_params.into_inner(),
        &auth_user,
    )
    .await?;
    Ok(Post::auth_response_accepted(json))
}

async fn resend_inner(
    log: &Logger,
    context: &ApiContext,
    path_params: OrgInviteParams,
    auth_user: &AuthUser,
) -> Result<JsonInvite, HttpError> {
    let query_organization = QueryOrganization::is_allowed_resource_id(
        auth_conn!(context),
        &context.rbac,
        &path_params.organization,
        auth_user,
        Permission::CreateRole,
    )?;
    let query_invite = QueryInvite::from_uuid(
        auth_conn!(context),
        query_organization.id,
        path_params.invite,
    )?;
    #[cfg(feature = "plus")]
    auth_user.rate_limit_invites(context)?;

    let (query_invite, secret) =
        query_invite.renew(write_conn!(context), auth_user.id, context.clock.now())?;
    email_invite(
        log,
        context,
        &query_organization,
        &query_invite,
        &secret,
        auth_user,
        None,
    )
    .await?;
    Ok(query_invite.into_json(auth_user.user.clone().into_pub_json()))
}

/// Refuse a credential that may not send an invitation: one that is not a browser session, or a
/// session that has not signed in within the last 10 minutes, since an invitation grants a role.
pub(crate) async fn check_inviter(
    context: &ApiContext,
    auth_user: &AuthUser,
) -> Result<(), HttpError> {
    // Checked only after authentication: an invalid credential still gets the opaque `401`.
    if !auth_user.is_session() {
        return Err(forbidden_error(ONLY_A_SESSION));
    }
    auth_user.check_fresh_sign_in(context).await
}

/// Email the invitation's link, which carries `secret` in its fragment, from `inviter`, and wait
/// until it is sent. The greeting names the account the address has, or else `name`.
pub(crate) async fn email_invite(
    log: &Logger,
    context: &ApiContext,
    query_organization: &QueryOrganization,
    query_invite: &QueryInvite,
    secret: &InviteSecret,
    inviter: &QueryUser,
    name: Option<UserName>,
) -> Result<(), HttpError> {
    let name = QueryUser::find_with_email(auth_conn!(context), &query_invite.email)
        .map_err(|e| {
            issue_error(
                "Failed to look up a user",
                "Failed to look up the user an invitation is for:",
                e,
            )
        })?
        .map(|user| user.name)
        .or(name);
    let mut button_url = context.console_url.join(INVITE_PATH).map_err(|e| {
        issue_error(
            "Failed to make an invitation link",
            "Failed to make an invitation link:",
            e,
        )
    })?;
    button_url.set_fragment(Some(&format!("invite={}", secret.as_ref())));

    let org_name = &query_organization.name;
    let role = query_invite.role;
    let body = Body::Button(Box::new(ButtonBody {
        title: format!("Invitation to join {org_name}"),
        preheader: "Click the provided link to join.".into(),
        greeting: if let Some(name) = name {
            format!("Ahoy {name}!")
        } else {
            "Ahoy!".into()
        },
        pre_body: format!(
            "Please, click the button below to accept the invitation from {inviter_name} ({inviter_email}) to join {org_name} as a {role} on Bencher.",
            inviter_name = inviter.name,
            inviter_email = inviter.email,
        ),
        button_text: format!("Join {org_name}"),
        button_url: button_url.into(),
        clipboard: None,
        post_body: format!(
            "The button works once, in the next {days} days.",
            days = INVITE_LIFETIME.num_days()
        ),
        closing: "See you soon,".into(),
        signature: "The Bencher Team".into(),
        settings_url: context
            .console_url
            .clone()
            .join("/help")
            .map(Into::into)
            .unwrap_or_default(),
    }));
    let message = Message {
        to_name: None,
        to_email: query_invite.email.to_string(),
        subject: Some(format!("Invitation to join {org_name}")),
        body: Some(body),
    };
    context
        .messenger
        .send_and_wait(log, message)
        .await
        .map_err(|e| {
            slog::error!(log, "Failed to send an invitation: {e}");
            service_unavailable_error(format!("Failed to send the invitation: {e}"))
        })?;

    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserInvite);

    Ok(())
}
