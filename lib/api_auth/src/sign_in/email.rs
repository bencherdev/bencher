use bencher_endpoint::{CorsResponse, Delete, Endpoint, Post, ResponseAccepted, ResponseDeleted};
use bencher_json::{
    Email, JsonEmailResend, JsonEmailSignIn, JsonSignInEmail, Jwt, OrganizationUuid, SignInRefusal,
};
#[cfg(feature = "plus")]
use bencher_json::{OAuthAuthorizationUuid, PlanLevel};
use bencher_schema::{
    context::ApiContext,
    error::{bad_request_error, issue_error, too_many_requests},
    model::{
        organization::QueryOrganization,
        user::{
            session::cookie::PendingCookie,
            sign_in::{QueryPendingSignIn, SignInContext},
        },
    },
    public_conn, write_conn,
};
use dropshot::{HttpError, RequestContext, TypedBody, endpoint};
use http::header::{RETRY_AFTER, SET_COOKIE};

use super::{Start, admit, admit_email, at_cap, binding, capped, end_replaced, ended, refused};

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
        &email,
        invite,
        claim,
        #[cfg(feature = "plus")]
        plan,
        #[cfg(feature = "plus")]
        authorization,
    )
    .await?;
    if let Some(wait) = at_cap(context, &email) {
        return Ok(capped(email, wait));
    }

    let sent = Start::insert(context, email, sign_in_context, context.clock.now())
        .await?
        .send(log, context)
        .await?;
    if let Some(earlier) = binding(context, &rqctx.request) {
        end_replaced(context, |conn| {
            QueryPendingSignIn::end_binding(conn, &earlier)
        })
        .await;
    }
    sent.answer(context)
}

/// What a sign-in carries to its proof, each part checked as it starts.
async fn sign_in_context(
    context: &ApiContext,
    email: &Email,
    invite: Option<Jwt>,
    claim: Option<OrganizationUuid>,
    #[cfg(feature = "plus")] plan: Option<PlanLevel>,
    #[cfg(feature = "plus")] authorization: Option<OAuthAuthorizationUuid>,
) -> Result<SignInContext, HttpError> {
    let unknown_organization = |e| refused(e, SignInRefusal::UnknownOrganization);
    if let Some(invite) = &invite {
        let claims = context.token_key.validate_invite(invite).map_err(|e| {
            refused(
                bad_request_error(format!("Invalid invitation: {e}")),
                SignInRefusal::InvalidInvite,
            )
        })?;
        if claims.email() != email {
            return Err(refused(
                bad_request_error("The invitation is for another email"),
                SignInRefusal::InviteForAnotherEmail,
            ));
        }
        QueryOrganization::from_uuid(public_conn!(context), claims.org.uuid)
            .map_err(unknown_organization)?;
    }
    if let Some(claim) = claim {
        QueryOrganization::from_uuid(public_conn!(context), claim).map_err(unknown_organization)?;
    }
    Ok(SignInContext {
        invite,
        claim,
        #[cfg(feature = "plus")]
        plan,
        #[cfg(feature = "plus")]
        authorization: crate::pending_authorization(context, authorization).await?,
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
    let wait = pending.resend_opens().timestamp() - now.timestamp();
    if wait > 0 {
        return Err(resend_waiting(wait));
    }
    if let Some(wait) = at_cap(context, &pending.email) {
        return Ok(capped(pending.email, wait));
    }

    let sent = Start::insert(context, pending.email.clone(), pending.context(), now)
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
