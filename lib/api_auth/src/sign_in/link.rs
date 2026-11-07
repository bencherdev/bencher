use bencher_endpoint::{CorsResponse, Endpoint, Post, ResponseOk};
use bencher_json::{BindingSecretHash, JsonSignInAs, JsonSignInLink};
use bencher_schema::{
    context::ApiContext, error::issue_error, model::user::sign_in::QueryPendingSignIn, public_conn,
    write_transaction,
};
use dropshot::{HttpError, RequestContext, TypedBody, endpoint};
use http::header::SET_COOKIE;

use super::{Presented, TxError, admit, binding, ended, finish, fresh, prove};

#[endpoint {
    method = OPTIONS,
    path = "/v0/auth/link",
    unpublished = true,
}]
pub async fn auth_link_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Post.into()]))
}

/// Sign in with a link
///
/// Sign in with the link from the email: at once in the browser that asked for it, and in any
/// other only once the reader confirms the address it names. Only in the browser that asked does
/// it pin the provider account a sign-in carries. A fresh sign-in's link proves its session again
/// in the browser that asked for it, and in no other.
#[endpoint {
    method = POST,
    path = "/v0/auth/link",
    unpublished = true,
}]
pub async fn auth_link_post(
    rqctx: RequestContext<ApiContext>,
    body: TypedBody<JsonSignInLink>,
) -> Result<ResponseOk<JsonSignInAs>, HttpError> {
    let log = &rqctx.log;
    let context = rqctx.context();
    admit(log, context, &rqctx.request)?;
    let JsonSignInLink { secret, confirm } = body.into_inner();
    let now = context.clock.now();

    let pending = QueryPendingSignIn::from_link(public_conn!(context), &secret, now)
        .map_err(|e| {
            issue_error(
                "Failed to find a sign-in",
                "Failed to find a pending sign-in by its link:",
                e,
            )
        })?
        .ok_or_else(ended)?;
    let asked_here = binding(context, &rqctx.request)
        .is_some_and(|binding| BindingSecretHash::from(&binding) == pending.binding_hash);
    if pending.session_id.is_some() {
        return fresh::link(context, &secret, asked_here, pending.email).await;
    }
    if !asked_here && !confirm {
        return Ok(Post::pub_response_ok(JsonSignInAs {
            email: pending.email,
            signed_in: None,
            proved: None,
        }));
    }

    let presented = Presented::new(context, &rqctx.request);
    let proof = write_transaction!(context, |conn| {
        // A fresh sign-in's link proves a session again and never signs in.
        let Some(pending) = QueryPendingSignIn::from_link(conn, &secret, now)?
            .filter(|pending| pending.session_id.is_none())
        else {
            return Ok(None);
        };
        // Only the browser that asked ran the provider's sign-in, so a link confirmed in any other
        // proves the address alone.
        #[cfg(feature = "plus")]
        let pending = if asked_here {
            pending
        } else {
            QueryPendingSignIn {
                pin_provider: None,
                pin_account: None,
                pin_handle: None,
                ..pending
            }
        };
        prove(conn, context, &pending, &presented, now).map(Some)
    })
    .map_err(TxError::into_http)?
    .ok_or_else(ended)?;
    let (signed_in, cookies) = finish(log, context, proof).await?;
    let mut response = Post::pub_response_ok(JsonSignInAs {
        email: pending.email,
        signed_in: Some(signed_in),
        proved: None,
    });
    for cookie in cookies {
        response.headers_mut().append(SET_COOKIE, cookie);
    }
    Ok(response)
}
