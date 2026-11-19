use bencher_endpoint::{CorsResponse, Delete, Endpoint, Get, ResponseDeleted, ResponseOk};
use bencher_json::JsonSession;
use bencher_schema::{
    auth_conn,
    context::ApiContext,
    error::{forbidden_error, issue_error},
    model::user::{
        auth::{AuthSession, AuthUser, BearerToken, dead_session},
        session::{QuerySession, cookie::SessionCookie},
    },
    write_conn,
};
use dropshot::{HttpError, RequestContext, endpoint};
use http::header::SET_COOKIE;

const NOT_A_SESSION: &str = "Only a browser session can read or end itself";

#[endpoint {
    method = OPTIONS,
    path = "/v0/auth/session",
    unpublished = true,
}]
pub async fn auth_session_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Delete.into()]))
}

/// View the session
///
/// View the signed-in reader's browser session: its user, how they signed in, and when it ends.
#[endpoint {
    method = GET,
    path = "/v0/auth/session",
    unpublished = true,
}]
pub async fn auth_session_get(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
) -> Result<ResponseOk<JsonSession>, HttpError> {
    let context = rqctx.context();
    let auth_user = AuthUser::from_token(context, bearer_token).await?;
    let session = session_of(&auth_user)?;
    let query_session = QuerySession::find_live(auth_conn!(context), session.id, session.uuid)
        .map_err(|e| issue_error("Failed to view a session", "Failed to view a session:", e))?
        .ok_or_else(|| dead_session(&context.console_url))?;
    let json = query_session.into_json(auth_user.user.into_json());
    Ok(Get::auth_response_ok(json))
}

/// End the session
///
/// End the signed-in reader's browser session and clear its cookie.
#[endpoint {
    method = DELETE,
    path = "/v0/auth/session",
    unpublished = true,
}]
pub async fn auth_session_delete(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
) -> Result<ResponseDeleted, HttpError> {
    let context = rqctx.context();
    let auth_user = AuthUser::from_token(context, bearer_token).await?;
    let session = session_of(&auth_user)?;
    QuerySession::delete(write_conn!(context), session.id, session.uuid)
        .map_err(|e| issue_error("Failed to end a session", "Failed to end a session:", e))?;
    let mut response = Delete::auth_response_deleted();
    response
        .headers_mut()
        .append(SET_COOKIE, SessionCookie::new(&context.console_url).clear());
    Ok(response)
}

fn session_of(auth_user: &AuthUser) -> Result<AuthSession, HttpError> {
    auth_user
        .session()
        .ok_or_else(|| forbidden_error(NOT_A_SESSION))
}
