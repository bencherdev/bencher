//! An invitation, read and accepted by the secret its link carries: the console reads one to fill
//! in the door's address, and a signed-in reader accepts one with a press.

use bencher_endpoint::{CorsResponse, Endpoint, Post, ResponseOk};
use bencher_json::{
    JsonInvitation, JsonInviteSecret, JsonOrganization, JsonOrganizationName, SignInRefusal,
};
use bencher_schema::{
    auth_conn,
    context::ApiContext,
    error::{bad_request_error, forbidden_error, issue_error},
    model::{
        organization::invite::{Accepted, QueryInvite},
        user::{
            auth::{AuthUser, BearerToken},
            session::cookie::CookieCredential,
        },
    },
    public_conn, write_transaction,
};
use dropshot::{HttpError, RequestContext, TypedBody, endpoint};

use crate::sign_in::{admit, refused};

/// One message for an invitation that has ended, expired, or never was, so none tells why.
const INVALID_INVITE: &str = "This invitation is not valid";
const INVITE_FOR_ANOTHER_EMAIL: &str = "This invitation is for another email";
const NOT_A_SESSION: &str = "Only a browser session can accept an invitation";

#[endpoint {
    method = OPTIONS,
    path = "/v0/auth/invite",
    unpublished = true,
}]
pub async fn auth_invite_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Post.into()]))
}

/// Read an invitation
///
/// What the invitation the secret names offers: the email it is for, its organization, its role,
/// and when it stops working; and, to a browser session, whether it is for the session's user.
#[endpoint {
    method = POST,
    path = "/v0/auth/invite",
    unpublished = true,
}]
pub async fn auth_invite_post(
    rqctx: RequestContext<ApiContext>,
    body: TypedBody<JsonInviteSecret>,
) -> Result<ResponseOk<JsonInvitation>, HttpError> {
    let context = rqctx.context();
    admit(&rqctx.log, context, &rqctx.request)?;
    let JsonInviteSecret { secret } = body.into_inner();
    let now = context.clock.now();
    let (query_invite, query_organization) =
        QueryInvite::from_secret(public_conn!(context), &secret, now)
            .map_err(|e| {
                issue_error(
                    "Failed to read an invitation",
                    "Failed to find an invitation by its secret:",
                    e,
                )
            })?
            .ok_or_else(invalid_invite)?;
    let for_you = session_user(context, CookieCredential::from_request(&rqctx.request))
        .await?
        .map(|auth_user| auth_user.user.email == query_invite.email);
    Ok(Post::pub_response_ok(JsonInvitation {
        email: query_invite.email,
        organization: JsonOrganizationName {
            name: query_organization.name,
            slug: query_organization.slug,
        },
        role: query_invite.role,
        expiration: query_invite.expiration,
        for_you,
    }))
}

/// The user of the browser session the cookie names, if the API honors it and it has not ended.
async fn session_user(
    context: &ApiContext,
    cookie: Option<CookieCredential>,
) -> Result<Option<AuthUser>, HttpError> {
    let Some(session) = cookie.map(BearerToken::Session) else {
        return Ok(None);
    };
    if !session.is_honored(context) {
        return Ok(None);
    }
    AuthUser::from_pub_token(context, session).await
}

#[endpoint {
    method = OPTIONS,
    path = "/v0/auth/invite/accept",
    unpublished = true,
}]
pub async fn auth_invite_accept_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Post.into()]))
}

/// Accept an invitation
///
/// Join the organization of the invitation the secret names, with its role, as the signed-in
/// reader of a browser session whose email the invitation is for. The invitation then ends. A
/// reader who already belongs to the organization ends it all the same.
#[endpoint {
    method = POST,
    path = "/v0/auth/invite/accept",
    unpublished = true,
}]
pub async fn auth_invite_accept_post(
    rqctx: RequestContext<ApiContext>,
    body: TypedBody<JsonInviteSecret>,
) -> Result<ResponseOk<JsonOrganization>, HttpError> {
    let context = rqctx.context();
    admit(&rqctx.log, context, &rqctx.request)?;
    let auth_user = AuthUser::new(&rqctx).await?;
    if auth_user.session_id().is_none() {
        return Err(forbidden_error(NOT_A_SESSION));
    }
    let JsonInviteSecret { secret } = body.into_inner();
    let now = context.clock.now();

    let accepted = write_transaction!(context, |conn| {
        let Some((query_invite, query_organization)) =
            QueryInvite::from_secret(conn, &secret, now)?
        else {
            return diesel::QueryResult::Ok(None);
        };
        let accepted = query_invite.accept(conn, &auth_user.user, now)?;
        Ok(Some((accepted, query_organization)))
    })
    .map_err(|e| {
        issue_error(
            "Failed to accept an invitation",
            "Failed to accept an invitation:",
            e,
        )
    })?;
    let query_organization = match accepted {
        Some((Accepted::Joined, query_organization)) => {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserAccept(None));
            query_organization
        },
        Some((Accepted::AlreadyMember, query_organization)) => query_organization,
        Some((Accepted::ForAnotherEmail, _)) => return Err(invite_for_another_email()),
        Some((Accepted::Ended, _)) | None => return Err(invalid_invite()),
    };
    Ok(Post::auth_response_ok(
        query_organization.into_json(auth_conn!(context)),
    ))
}

/// The one refusal of an invitation that has ended, expired, or never was.
pub(crate) fn invalid_invite() -> HttpError {
    refused(
        bad_request_error(INVALID_INVITE),
        SignInRefusal::InvalidInvite,
    )
}

/// The refusal of an invitation for an address other than the one it is offered for.
pub(crate) fn invite_for_another_email() -> HttpError {
    refused(
        bad_request_error(INVITE_FOR_ANOTHER_EMAIL),
        SignInRefusal::InviteForAnotherEmail,
    )
}
