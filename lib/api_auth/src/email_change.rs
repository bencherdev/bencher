//! An email change, read and confirmed by the secret its link carries: the console reads one to
//! show both addresses, and a press confirms it, in the session that asked for it alone, since the
//! link proves the new address and the session proves who asked.

use bencher_endpoint::{CorsResponse, Endpoint, Post, ResponseOk};
use bencher_json::{JsonEmailChange, JsonEmailChangeSecret, SignInRefusal};
#[cfg(feature = "plus")]
use bencher_schema::model::organization::sso::QuerySso;
use bencher_schema::{
    context::ApiContext,
    error::{
        BencherResource, bad_request_error, forbidden_error, issue_error, resource_conflict_error,
    },
    model::user::{
        auth::BearerToken,
        email_change::{Confirmation, Found, QueryEmailChange},
    },
    public_conn, write_transaction,
};
use dropshot::{HttpError, RequestContext, SharedExtractor as _, TypedBody, endpoint};

use crate::{
    invite::session_user,
    sign_in::{admit, locked_refusal, refused},
};

/// One message for a change that has expired, was asked for again, or never was, so none tells
/// why.
const EXPIRED: &str = "This email change has expired";
const USED: &str = "This email change was already confirmed";
const ELSEWHERE: &str =
    "An email change is confirmed only in the browser session that asked for it";
const TAKEN: &str = "The email is already in use";

#[endpoint {
    method = OPTIONS,
    path = "/v0/auth/email/change",
    unpublished = true,
}]
pub async fn auth_email_change_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Post.into()]))
}

/// Read an email change
///
/// The user's email and the new email of the change the secret names, while it waits for its
/// confirm.
#[endpoint {
    method = POST,
    path = "/v0/auth/email/change",
    unpublished = true,
}]
pub async fn auth_email_change_post(
    rqctx: RequestContext<ApiContext>,
    body: TypedBody<JsonEmailChangeSecret>,
) -> Result<ResponseOk<JsonEmailChange>, HttpError> {
    let context = rqctx.context();
    admit(&rqctx.log, context, &rqctx.request)?;
    let JsonEmailChangeSecret { secret } = body.into_inner();
    let found = QueryEmailChange::find(public_conn!(context), &secret, context.clock.now())
        .map_err(|e| {
            issue_error(
                "Failed to read an email change",
                "Failed to find an email change by its secret:",
                e,
            )
        })?;
    match found {
        Found::Live(change, _user) => Ok(Post::pub_response_ok(JsonEmailChange {
            email: change.email,
            new_email: change.new_email,
        })),
        Found::Confirmed => Err(used()),
        Found::Expired => Err(expired()),
    }
}

#[endpoint {
    method = OPTIONS,
    path = "/v0/auth/email/change/confirm",
    unpublished = true,
}]
pub async fn auth_email_change_confirm_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Post.into()]))
}

/// Confirm an email change
///
/// Make the new email of the change the secret names the user's, proven now, and answer with the
/// change, in the browser session that asked for it and in no other. The user's deprecated API
/// tokens are revoked, and every session of theirs ends but the one that asked for the change.
#[endpoint {
    method = POST,
    path = "/v0/auth/email/change/confirm",
    unpublished = true,
}]
pub async fn auth_email_change_confirm_post(
    rqctx: RequestContext<ApiContext>,
    body: TypedBody<JsonEmailChangeSecret>,
) -> Result<ResponseOk<JsonEmailChange>, HttpError> {
    let context = rqctx.context();
    admit(&rqctx.log, context, &rqctx.request)?;
    // A bearer credential wins over the cookie, and no bearer credential is a browser session.
    let cookie = match BearerToken::from_request(&rqctx).await {
        Ok(BearerToken::Session(cookie)) => Some(cookie),
        Ok(BearerToken::Jwt(_) | BearerToken::UserKey(_)) | Err(_) => None,
    };
    let presented = session_user(context, cookie)
        .await?
        .and_then(|auth_user| auth_user.session_id());
    let JsonEmailChangeSecret { secret } = body.into_inner();
    let now = context.clock.now();
    let confirmation = write_transaction!(context, |conn| QueryEmailChange::confirm(
        conn, &secret, presented, now
    ))
    .map_err(|e| {
        issue_error(
            "Failed to confirm an email change",
            "Failed to confirm an email change:",
            e,
        )
    })?;
    let (change, query_user) = match confirmation {
        Confirmation::Confirmed(change, query_user) => (change, query_user),
        Confirmation::Used => return Err(used()),
        Confirmation::Expired => return Err(expired()),
        Confirmation::Locked => return Err(locked_refusal(context)),
        Confirmation::Elsewhere => return Err(elsewhere()),
        Confirmation::Taken(new_email) => {
            return Err(resource_conflict_error(
                BencherResource::User,
                new_email,
                TAKEN,
            ));
        },
    };

    #[cfg(feature = "plus")]
    QuerySso::join_all(
        context,
        &query_user,
        #[cfg(feature = "otel")]
        bencher_otel::AuthMethod::Email,
    )
    .await?;
    #[cfg(not(feature = "plus"))]
    drop(query_user);

    Ok(Post::pub_response_ok(JsonEmailChange {
        email: change.email,
        new_email: change.new_email,
    }))
}

/// The one refusal of a change that has expired, was asked for again, or never was.
fn expired() -> HttpError {
    refused(
        bad_request_error(EXPIRED),
        SignInRefusal::EmailChangeExpired,
    )
}

fn used() -> HttpError {
    refused(bad_request_error(USED), SignInRefusal::EmailChangeUsed)
}

fn elsewhere() -> HttpError {
    refused(
        forbidden_error(ELSEWHERE),
        SignInRefusal::EmailChangeElsewhere,
    )
}
