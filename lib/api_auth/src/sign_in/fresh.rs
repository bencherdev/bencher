//! A fresh sign-in: the signed-in reader proves who they are again, by an email to their own
//! address or by a provider pinned to them, which moves the session's last proof to now and makes
//! nothing else.

use bencher_endpoint::{CorsResponse, Endpoint, Get, Post, ResponseAccepted, ResponseOk};
use bencher_json::{
    DateTime, Email, JsonFreshSignIn, JsonProved, JsonSignInAs, JsonSignInCode, JsonSignInEmail,
    LinkSecret, SignInRefusal,
};
use bencher_schema::{
    context::{ApiContext, DbConnection},
    error::{forbidden_error, issue_error},
    model::user::{
        auth::AuthUser,
        session::{QuerySession, SessionId, cookie::PendingCookie},
        sign_in::QueryPendingSignIn,
    },
    public_conn, write_transaction,
};
use dropshot::{HttpError, RequestContext, TypedBody, endpoint};
use http::header::SET_COOKIE;

use super::{
    Emailed, TxError, admit, binding, capped,
    code::{Redeemed, redeem},
    email::resend,
    email_fresh, ended, refused,
};

const NOT_A_SESSION: &str = "Only a browser session can sign in again";
const ASKED_ELSEWHERE: &str = "A fresh sign-in's link proves only the browser that asked for it";

#[endpoint {
    method = OPTIONS,
    path = "/v0/auth/fresh",
    unpublished = true,
}]
pub async fn auth_fresh_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

/// How to sign in again
///
/// The ways the signed-in reader can prove who they are again: an email to their address, and
/// each provider pinned to them that this server signs in with.
#[endpoint {
    method = GET,
    path = "/v0/auth/fresh",
    unpublished = true,
}]
pub async fn auth_fresh_get(
    rqctx: RequestContext<ApiContext>,
) -> Result<ResponseOk<JsonFreshSignIn>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    session_of(&auth_user)?;
    Ok(Get::auth_response_ok(JsonFreshSignIn {
        #[cfg(feature = "plus")]
        providers: crate::provider::pinned(rqctx.context(), &auth_user).await?,
    }))
}

#[endpoint {
    method = OPTIONS,
    path = "/v0/auth/fresh/email",
    unpublished = true,
}]
pub async fn auth_fresh_email_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Post.into()]))
}

/// Sign in again by email
///
/// Email the signed-in reader's address a code and a link that prove this session again, in this
/// browser alone.
#[endpoint {
    method = POST,
    path = "/v0/auth/fresh/email",
    unpublished = true,
}]
pub async fn auth_fresh_email_post(
    rqctx: RequestContext<ApiContext>,
) -> Result<ResponseAccepted<JsonSignInEmail>, HttpError> {
    let log = &rqctx.log;
    let context = rqctx.context();
    admit(log, context, &rqctx.request)?;
    let auth_user = AuthUser::new(&rqctx).await?;
    let session_id = session_of(&auth_user)?;
    let email = auth_user.user.email.clone();
    match email_fresh(log, context, &rqctx.request, email, session_id).await? {
        Emailed::Sent(sent) => sent.answer(context),
        Emailed::Capped { email, wait } => Ok(capped(email, wait)),
    }
}

#[endpoint {
    method = OPTIONS,
    path = "/v0/auth/fresh/email/resend",
    unpublished = true,
}]
pub async fn auth_fresh_email_resend_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Post.into()]))
}

/// Resend the fresh sign-in email
///
/// End this session's fresh sign-in in this browser and email another, with a new code and link,
/// once 60 seconds have passed since it started.
#[endpoint {
    method = POST,
    path = "/v0/auth/fresh/email/resend",
    unpublished = true,
}]
pub async fn auth_fresh_email_resend_post(
    rqctx: RequestContext<ApiContext>,
) -> Result<ResponseAccepted<JsonSignInEmail>, HttpError> {
    let log = &rqctx.log;
    let context = rqctx.context();
    admit(log, context, &rqctx.request)?;
    let auth_user = AuthUser::new(&rqctx).await?;
    let session_id = session_of(&auth_user)?;
    let binding = binding(context, &rqctx.request).ok_or_else(ended)?;
    let now = context.clock.now();
    let pending =
        QueryPendingSignIn::fresh_from_binding(public_conn!(context), &binding, session_id, now)
            .map_err(|e| {
                issue_error(
                    "Failed to find a sign-in",
                    "Failed to find a fresh sign-in:",
                    e,
                )
            })?
            .ok_or_else(ended)?;
    resend(log, context, pending, now).await
}

#[endpoint {
    method = OPTIONS,
    path = "/v0/auth/fresh/code",
    unpublished = true,
}]
pub async fn auth_fresh_code_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Post.into()]))
}

/// Sign in again with a code
///
/// Prove this session again with the code from the email, in the session and the browser that
/// asked for it. A wrong code counts a try and spends the address's budget, as a sign-in's does.
#[endpoint {
    method = POST,
    path = "/v0/auth/fresh/code",
    unpublished = true,
}]
pub async fn auth_fresh_code_post(
    rqctx: RequestContext<ApiContext>,
    body: TypedBody<JsonSignInCode>,
) -> Result<ResponseOk<JsonProved>, HttpError> {
    let log = &rqctx.log;
    let context = rqctx.context();
    admit(log, context, &rqctx.request)?;
    let auth_user = AuthUser::new(&rqctx).await?;
    let session_id = session_of(&auth_user)?;
    let JsonSignInCode { code } = body.into_inner();
    let binding = binding(context, &rqctx.request).ok_or_else(ended)?;
    let now = context.clock.now();

    write_transaction!(context, |conn| {
        let Some(pending) =
            QueryPendingSignIn::fresh_from_binding(conn, &binding, session_id, now)?
        else {
            return Ok(Redeemed::Ended);
        };
        redeem(conn, context, &pending, &code, now, |conn| {
            prove_again(conn, &pending, session_id, now)
        })
    })
    .map_err(TxError::into_http)?
    .proof(context, now)?
    .then_some(())
    .ok_or_else(ended)?;
    let mut response = Post::auth_response_ok(JsonProved { proved: now });
    response
        .headers_mut()
        .append(SET_COOKIE, PendingCookie::new(&context.console_url).clear());
    Ok(response)
}

/// Prove a fresh sign-in's session again by its link, at once in the browser that asked for it,
/// and never in any other, which changes nothing.
pub(super) async fn link(
    context: &ApiContext,
    secret: &LinkSecret,
    asked_here: bool,
    email: Email,
) -> Result<ResponseOk<JsonSignInAs>, HttpError> {
    if !asked_here {
        return Err(refused(
            forbidden_error(ASKED_ELSEWHERE),
            SignInRefusal::AskedElsewhere,
        ));
    }
    let now = context.clock.now();
    let proved = write_transaction!(context, |conn| {
        let Some(pending) = QueryPendingSignIn::from_link(conn, secret, now)? else {
            return Ok(false);
        };
        let Some(session_id) = pending.session_id else {
            return Ok(false);
        };
        prove_again(conn, &pending, session_id, now)
    })
    .map_err(TxError::into_http)?;
    if !proved {
        return Err(ended());
    }
    let mut response = Post::pub_response_ok(JsonSignInAs {
        email,
        signed_in: None,
        proved: Some(now),
    });
    response
        .headers_mut()
        .append(SET_COOKIE, PendingCookie::new(&context.console_url).clear());
    Ok(response)
}

/// End the fresh sign-in, so neither its code nor its link works again, and move the last proof
/// of `session_id`, the session it was asked for, to `now`; whether it proved that session. A row
/// asked for any other session, or for none, proves nothing and ends nothing.
fn prove_again(
    conn: &mut DbConnection,
    pending: &QueryPendingSignIn,
    session_id: SessionId,
    now: DateTime,
) -> Result<bool, TxError> {
    if pending.session_id != Some(session_id) {
        return Ok(false);
    }
    QueryPendingSignIn::end(conn, pending.id)?;
    Ok(QuerySession::prove(conn, session_id, now)? == 1)
}

/// The browser session the reader signed in to, which alone can sign in again.
pub(crate) fn session_of(auth_user: &AuthUser) -> Result<SessionId, HttpError> {
    auth_user
        .session_id()
        .ok_or_else(|| forbidden_error(NOT_A_SESSION))
}
