use bencher_endpoint::{CorsResponse, Endpoint, Get, Post};
use bencher_json::{Email, OAuthScope, OAuthScopes, UserName, UserSlug, UserUuid};
use bencher_schema::{
    context::ApiContext,
    model::user::{QueryUser, oauth::authenticate},
    public_conn,
};
use dropshot::{Body, HttpError, RequestContext, endpoint};
use http::{Response, StatusCode};
use serde::Serialize;

use crate::{
    openid_provider,
    response::{bearer_error, json, oauth_error},
};

#[endpoint {
    method = OPTIONS,
    path = "/oauth/userinfo",
    unpublished = true,
}]
pub async fn oauth_userinfo_options(
    rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    openid_provider(rqctx.context())?;
    Ok(Endpoint::cors(&[Get.into(), Post.into()]))
}

/// OIDC user info endpoint
///
/// Who the user of an access token with the `openid` scope is, as far as its scopes tell.
#[endpoint {
    method = GET,
    path = "/oauth/userinfo",
    unpublished = true,
}]
pub async fn oauth_userinfo_get(
    rqctx: RequestContext<ApiContext>,
) -> Result<Response<Body>, HttpError> {
    userinfo(&rqctx).await
}

/// OIDC user info endpoint
///
/// Who the user of an access token with the `openid` scope is, as far as its scopes tell.
#[endpoint {
    method = POST,
    path = "/oauth/userinfo",
    unpublished = true,
}]
pub async fn oauth_userinfo_post(
    rqctx: RequestContext<ApiContext>,
) -> Result<Response<Body>, HttpError> {
    userinfo(&rqctx).await
}

async fn userinfo(rqctx: &RequestContext<ApiContext>) -> Result<Response<Body>, HttpError> {
    let context = rqctx.context();
    let (oauth, _signing_key) = openid_provider(context)?;
    let Some(token) = rqctx
        .request
        .headers()
        .get(bencher_json::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(bencher_json::strip_bearer_token)
    else {
        return bearer_error(
            StatusCode::UNAUTHORIZED,
            "Bearer",
            "invalid_request",
            "An access token is required",
        );
    };
    let Ok((query_user, query_grant)) = authenticate(context, public_conn!(context), oauth, token)
    else {
        return bearer_error(
            StatusCode::UNAUTHORIZED,
            r#"Bearer error="invalid_token", error_description="The access token is invalid""#,
            "invalid_token",
            "The access token is invalid",
        );
    };
    if context.rate_limiting.user_request(query_user.uuid).is_err() {
        return oauth_error(
            StatusCode::TOO_MANY_REQUESTS,
            "temporarily_unavailable",
            "Too many requests, please try again later",
            oauth.issuer(),
        );
    }
    if !query_grant.scope.permits(OAuthScope::Openid) {
        return bearer_error(
            StatusCode::FORBIDDEN,
            r#"Bearer error="insufficient_scope", scope="openid""#,
            "insufficient_scope",
            "The access token was not granted the openid scope",
        );
    }
    json(&Identity::new(&query_user, &query_grant.scope))
}

/// What a grant's scopes let its client know about the user: always who they are, by a UUID that
/// is never reassigned.
#[derive(Serialize)]
pub(crate) struct Identity<'u> {
    sub: UserUuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<&'u Email>,
    #[serde(skip_serializing_if = "Option::is_none")]
    email_verified: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<&'u UserName>,
    #[serde(skip_serializing_if = "Option::is_none")]
    preferred_username: Option<&'u UserSlug>,
}

impl<'u> Identity<'u> {
    pub(crate) fn new(query_user: &'u QueryUser, scope: &OAuthScopes) -> Self {
        let email = scope.permits(OAuthScope::Email);
        let profile = scope.permits(OAuthScope::Profile);
        Self {
            sub: query_user.uuid,
            email: email.then_some(&query_user.email),
            email_verified: email.then_some(query_user.email_verified.is_some()),
            name: profile.then_some(&query_user.name),
            preferred_username: profile.then_some(&query_user.slug),
        }
    }
}
