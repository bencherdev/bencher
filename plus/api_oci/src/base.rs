//! OCI Base Endpoint - GET /v2/
//!
//! Returns 200 OK if a valid OCI Bearer token is provided (auth or public).
//! Returns 401 with WWW-Authenticate if no token or invalid token.
//! This is the first endpoint clients call to verify registry compatibility.
//!
//! Runner tokens are not accepted here — the runner's OCI client
//! pulls directly from manifest/blob endpoints without hitting `/v2/`.

use bencher_endpoint::{CorsResponse, Endpoint, Get};
use bencher_schema::context::ApiContext;
use dropshot::{Body, HttpError, RequestContext, endpoint};
use http::Response;

use crate::auth::{
    apply_public_rate_limit, apply_user_rate_limit, extract_oci_bearer_token,
    unauthorized_with_www_authenticate,
};
use crate::response::{APPLICATION_JSON, EMPTY_JSON_BODY, oci_cors_headers};

/// CORS preflight for OCI base endpoint
#[endpoint {
    method = OPTIONS,
    path = "/v2/",
    tags = ["oci"],
    unpublished = true,
}]
pub async fn oci_base_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

/// OCI API version check endpoint
///
/// Returns 200 OK if a valid Bearer token is provided.
/// Returns 401 Unauthorized with WWW-Authenticate header if not authenticated.
/// Accepts auth (user) and public (anonymous) OCI tokens.
#[expect(
    clippy::map_err_ignore,
    reason = "Intentionally discarding auth errors for security"
)]
#[endpoint {
    method = GET,
    path = "/v2/",
    tags = ["oci"],
}]
pub async fn oci_base(rqctx: RequestContext<ApiContext>) -> Result<Response<Body>, HttpError> {
    let context = rqctx.context();

    // Extract Bearer token — required for all access
    let token = extract_oci_bearer_token(&rqctx)
        .map_err(|_| unauthorized_with_www_authenticate(&rqctx, None))?;

    // Auth tokens (most common — authenticated Docker users), then public tokens
    if let Ok(claims) = context.token_key.validate_oci_auth(&token) {
        apply_user_rate_limit(&rqctx.log, context, &claims).await?;
    } else if context.token_key.validate_oci_public(&token).is_ok() {
        apply_public_rate_limit(&rqctx.log, context, &rqctx)?;
    } else {
        return Err(unauthorized_with_www_authenticate(&rqctx, None));
    }

    // Return 200 OK with empty JSON body
    oci_cors_headers(
        Response::builder()
            .status(http::StatusCode::OK)
            .header(http::header::CONTENT_TYPE, APPLICATION_JSON),
        &[http::Method::GET],
    )
    .body(Body::from(EMPTY_JSON_BODY))
    .map_err(|e| HttpError::for_internal_error(format!("Failed to build response: {e}")))
}
