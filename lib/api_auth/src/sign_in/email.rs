use bencher_endpoint::{CorsResponse, Delete, Endpoint, Post, ResponseAccepted, ResponseDeleted};
use bencher_json::{
    DateTime, Email, InviteSecret, JsonEmailResend, JsonEmailSignIn, JsonSignInEmail,
    OrganizationUuid, SignInRefusal,
};
#[cfg(feature = "plus")]
use bencher_json::{OAuthAuthorizationUuid, PlanLevel};
use bencher_schema::{
    context::ApiContext,
    error::{issue_error, too_many_requests},
    model::{
        organization::{QueryOrganization, invite::QueryInvite},
        user::{
            session::cookie::PendingCookie,
            sign_in::{QueryPendingSignIn, SignInContext},
        },
    },
    public_conn, write_conn,
};
use dropshot::{HttpError, RequestContext, TypedBody, endpoint};
use http::header::{RETRY_AFTER, SET_COOKIE};
use slog::Logger;

use super::{
    Emailed, Start, admit, admit_email, at_cap, binding, capped, email_sign_in, end_replaced,
    ended, refused,
};
use crate::invite::{invalid_invite, invite_for_another_email};

#[endpoint {
    method = OPTIONS,
    path = "/v0/auth/email",
    unpublished = true,
}]
pub async fn auth_email_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Post.into(), Delete.into()]))
}

/// Sign in by email
///
/// Email a code and a link that sign this browser in to the address's account, or sign it up.
/// The answer is the same for every address, with an account or without one.
#[endpoint {
    method = POST,
    path = "/v0/auth/email",
    unpublished = true,
}]
pub async fn auth_email_post(
    rqctx: RequestContext<ApiContext>,
    body: TypedBody<JsonEmailSignIn>,
) -> Result<ResponseAccepted<JsonSignInEmail>, HttpError> {
    let log = &rqctx.log;
    let context = rqctx.context();
    let JsonEmailSignIn {
        email,
        invite,
        claim,
        #[cfg(feature = "plus")]
        plan,
        #[cfg(feature = "plus")]
        authorization,
        #[cfg(feature = "plus")]
        turnstile,
        #[cfg(not(feature = "plus"))]
            turnstile: _,
    } = body.into_inner();
    admit_email(
        log,
        context,
        &rqctx.request,
        #[cfg(feature = "plus")]
        turnstile.as_ref(),
    )
    .await?;

    let sign_in_context = sign_in_context(
        context,
        Some(&email),
        invite,
        claim,
        #[cfg(feature = "plus")]
        plan,
        #[cfg(feature = "plus")]
        authorization,
    )
    .await?;
    match email_sign_in(log, context, &rqctx.request, email, sign_in_context).await? {
        Emailed::Sent(sent) => sent.answer(context),
        Emailed::Capped { email, wait } => Ok(capped(email, wait)),
    }
}

/// What a sign-in carries to where it lands, each part checked as it starts: an invitation must
/// not have ended or expired, and, for a sign-in by `email`, must be for that address.
pub(crate) async fn sign_in_context(
    context: &ApiContext,
    email: Option<&Email>,
    invite: Option<InviteSecret>,
    claim: Option<OrganizationUuid>,
    #[cfg(feature = "plus")] plan: Option<PlanLevel>,
    #[cfg(feature = "plus")] authorization: Option<OAuthAuthorizationUuid>,
) -> Result<SignInContext, HttpError> {
    let invite = if let Some(secret) = &invite {
        let (query_invite, _query_organization) =
            QueryInvite::from_secret(public_conn!(context), secret, context.clock.now())
                .map_err(|e| {
                    issue_error(
                        "Failed to find an invitation",
                        "Failed to find the invitation a sign-in carries:",
                        e,
                    )
                })?
                .ok_or_else(invalid_invite)?;
        if email.is_some_and(|email| query_invite.email != *email) {
            return Err(invite_for_another_email());
        }
        Some(query_invite.id)
    } else {
        None
    };
    if let Some(claim) = claim {
        QueryOrganization::from_uuid(public_conn!(context), claim)
            .map_err(|e| refused(e, SignInRefusal::UnknownOrganization))?;
    }
    Ok(SignInContext {
        invite,
        claim,
        #[cfg(feature = "plus")]
        plan,
        #[cfg(feature = "plus")]
        authorization: crate::pending_authorization(context, authorization).await?,
        #[cfg(feature = "plus")]
        pin: None,
    })
}

/// Use another email
///
/// End this browser's pending sign-in and clear its cookie.
#[endpoint {
    method = DELETE,
    path = "/v0/auth/email",
    unpublished = true,
}]
pub async fn auth_email_delete(
    rqctx: RequestContext<ApiContext>,
) -> Result<ResponseDeleted, HttpError> {
    let context = rqctx.context();
    admit(&rqctx.log, context, &rqctx.request)?;
    if let Some(binding) = binding(context, &rqctx.request) {
        QueryPendingSignIn::end_binding(write_conn!(context), &binding).map_err(|e| {
            issue_error(
                "Failed to end a sign-in",
                "Failed to end a pending sign-in:",
                e,
            )
        })?;
    }
    let mut response = Delete::pub_response_deleted();
    response
        .headers_mut()
        .append(SET_COOKIE, PendingCookie::new(&context.console_url).clear());
    Ok(response)
}

#[endpoint {
    method = OPTIONS,
    path = "/v0/auth/email/resend",
    unpublished = true,
}]
pub async fn auth_email_resend_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Post.into()]))
}

/// Resend the sign-in email
///
/// End this browser's pending sign-in and start another for the same address, with a new code
/// and link, once 60 seconds have passed since it started.
#[endpoint {
    method = POST,
    path = "/v0/auth/email/resend",
    unpublished = true,
}]
pub async fn auth_email_resend_post(
    rqctx: RequestContext<ApiContext>,
    body: TypedBody<JsonEmailResend>,
) -> Result<ResponseAccepted<JsonSignInEmail>, HttpError> {
    let log = &rqctx.log;
    let context = rqctx.context();
    let JsonEmailResend {
        #[cfg(feature = "plus")]
        turnstile,
        #[cfg(not(feature = "plus"))]
            turnstile: _,
    } = body.into_inner();
    admit_email(
        log,
        context,
        &rqctx.request,
        #[cfg(feature = "plus")]
        turnstile.as_ref(),
    )
    .await?;

    let binding = binding(context, &rqctx.request).ok_or_else(ended)?;
    let now = context.clock.now();
    let pending = QueryPendingSignIn::from_binding(public_conn!(context), &binding, now)
        .map_err(|e| {
            issue_error(
                "Failed to find a sign-in",
                "Failed to find a pending sign-in:",
                e,
            )
        })?
        .ok_or_else(ended)?;
    resend(log, context, pending, now).await
}

/// End the pending sign-in and email another, which carries what it carries, once Resend opens.
pub(super) async fn resend(
    log: &Logger,
    context: &ApiContext,
    pending: QueryPendingSignIn,
    now: DateTime,
) -> Result<ResponseAccepted<JsonSignInEmail>, HttpError> {
    let wait = pending.resend_opens().timestamp() - now.timestamp();
    if wait > 0 {
        return Err(resend_waiting(wait));
    }
    if let Some(wait) = at_cap(context, &pending.email) {
        return Ok(capped(pending.email, wait));
    }

    let sent = Start::insert(context, pending.again(&context.code_key, now)?, now)
        .await?
        .send(log, context)
        .await?;
    // A proof or its last try may have ended the sign-in since it was read. Then Resend is refused
    // as ended, and the sign-in it started ends too, since the answer hands out no cookie for it.
    let replaced = end_replaced(context, |conn| {
        let ended = QueryPendingSignIn::end(conn, pending.id)?;
        if ended == 0 {
            sent.end(conn)?;
        }
        Ok(ended)
    })
    .await;
    if replaced == Some(0) {
        return Err(ended());
    }
    sent.answer(context)
}

/// The refusal of a Resend before it opens, `wait` seconds from now.
fn resend_waiting(wait: i64) -> HttpError {
    let mut error = refused(
        too_many_requests(format!("Resend opens in {wait} seconds")),
        SignInRefusal::ResendWaiting,
    );
    if let Err(e) = error.add_header(RETRY_AFTER, wait.to_string()) {
        return issue_error(
            "Failed to refuse a Resend",
            "Failed to set the Retry-After header of a Resend refusal:",
            e,
        );
    }
    error
}
