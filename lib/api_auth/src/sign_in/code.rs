use bencher_endpoint::{CorsResponse, Endpoint, Post, ResponseOk};
use bencher_json::{DateTime, JsonSignInCode, JsonSignedIn, SignInCode, SignInRefusal, TRIES_LEFT};
use bencher_schema::{
    context::{ApiContext, DbConnection},
    error::{forbidden_error, too_many_requests},
    model::user::{
        session::cookie::PendingCookie,
        sign_in::{CodeBudget, QueryPendingSignIn},
    },
    write_transaction,
};
use dropshot::{HttpError, RequestContext, TypedBody, endpoint};
use http::{
    HeaderValue,
    header::{RETRY_AFTER, SET_COOKIE},
};

use super::{Presented, TxError, admit, binding, ended, finish, prove, refused};

#[endpoint {
    method = OPTIONS,
    path = "/v0/auth/code",
    unpublished = true,
}]
pub async fn auth_code_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Post.into()]))
}

/// Sign in with a code
///
/// Sign in with the code from the email, in the browser that asked for it.
/// A wrong code counts a try, and the fifth ends the sign-in. After 10 wrong codes for an address
/// in 24 hours, its codes pause until the window has room, and its link still works.
#[endpoint {
    method = POST,
    path = "/v0/auth/code",
    unpublished = true,
}]
pub async fn auth_code_post(
    rqctx: RequestContext<ApiContext>,
    body: TypedBody<JsonSignInCode>,
) -> Result<ResponseOk<JsonSignedIn>, HttpError> {
    let log = &rqctx.log;
    let context = rqctx.context();
    admit(log, context, &rqctx.request)?;
    let JsonSignInCode { code } = body.into_inner();
    let binding = binding(context, &rqctx.request).ok_or_else(ended)?;
    let presented = Presented::new(context, &rqctx.request);
    let now = context.clock.now();

    let proof = write_transaction!(context, |conn| {
        let Some(pending) = QueryPendingSignIn::from_binding(conn, &binding, now)? else {
            return Ok(Redeemed::Ended);
        };
        redeem(conn, context, &pending, &code, now, |conn| {
            prove(conn, context, &pending, &presented, now)
        })
    })
    .map_err(TxError::into_http)?
    .proof(context, now)?;
    let (json, cookies) = finish(log, context, proof).await?;
    let mut response = Post::pub_response_ok(json);
    for cookie in cookies {
        response.headers_mut().append(SET_COOKIE, cookie);
    }
    Ok(response)
}

/// What a code did to its pending sign-in.
pub(super) enum Redeemed<T> {
    Ended,
    /// Codes for the address are paused until then.
    Paused(DateTime),
    /// A wrong code, with the tries it left.
    Wrong(i32),
    Proof(T),
}

impl<T> Redeemed<T> {
    /// The proof, or the refusal of a code that proved nothing.
    pub(super) fn proof(self, context: &ApiContext, now: DateTime) -> Result<T, HttpError> {
        match self {
            Self::Ended => Err(ended()),
            Self::Paused(resumes) => Err(codes_paused(resumes, now)),
            Self::Wrong(tries_left) => Err(wrong_code(context, tries_left)),
            Self::Proof(proof) => Ok(proof),
        }
    }
}

/// Compare `code` with the pending sign-in's, inside its write transaction, under its address's
/// budget: a wrong code spends the budget and a try, and the right one is proven by `prove`.
pub(super) fn redeem<T, F>(
    conn: &mut DbConnection,
    context: &ApiContext,
    pending: &QueryPendingSignIn,
    code: &SignInCode,
    now: DateTime,
    prove: F,
) -> Result<Redeemed<T>, TxError>
where
    F: FnOnce(&mut DbConnection) -> Result<T, TxError>,
{
    // Before the code is compared, so a paused code tells nothing, right or wrong, and spends no
    // try.
    if let Some(resumes) = CodeBudget::paused_until(conn, &pending.email, now)? {
        return Ok(Redeemed::Paused(resumes));
    }
    if !context.code_key.matches(code, &pending.code_hash) {
        CodeBudget::spend(conn, &pending.email, now)?;
        return Ok(Redeemed::Wrong(pending.count_wrong_code(conn)?));
    }
    prove(conn).map(Redeemed::Proof)
}

fn wrong_code(context: &ApiContext, tries_left: i32) -> HttpError {
    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::SignInRefused(
        bencher_otel::SignInRefusalReason::WrongCode,
    ));
    let (message, refusal) = match tries_left {
        0 => (
            "Wrong code. No tries left.".to_owned(),
            SignInRefusal::NoTriesLeft,
        ),
        1 => (
            "Wrong code. 1 try left.".to_owned(),
            SignInRefusal::WrongCode,
        ),
        tries_left => (
            format!("Wrong code. {tries_left} tries left."),
            SignInRefusal::WrongCode,
        ),
    };
    let mut error = refused(forbidden_error(message), refusal);
    let headers = error.headers_mut();
    headers.insert(TRIES_LEFT, HeaderValue::from(tries_left));
    if tries_left == 0 {
        headers.append(SET_COOKIE, PendingCookie::new(&context.console_url).clear());
    }
    error
}

/// The refusal of a code while its address's budget is spent, until codes resume.
fn codes_paused(resumes: DateTime, now: DateTime) -> HttpError {
    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::SignInRefused(
        bencher_otel::SignInRefusalReason::CodesPaused,
    ));
    let mut error = refused(
        too_many_requests("Codes for this address are paused. Sign in with the link in the email."),
        SignInRefusal::CodesPaused,
    );
    error.headers_mut().insert(
        RETRY_AFTER,
        HeaderValue::from(resumes.timestamp() - now.timestamp()),
    );
    error
}
