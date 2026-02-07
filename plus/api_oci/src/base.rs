//! OCI Base Endpoint - GET /v2/
//!
//! Returns 200 OK if authenticated, 401 with WWW-Authenticate if not.
//! This is the first endpoint clients call to verify registry compatibility.

use bencher_endpoint::{CorsResponse, Endpoint, Get};
use bencher_schema::context::ApiContext;
use dropshot::{Body, HttpError, RequestContext, endpoint};
use http::Response;

#[cfg(feature = "plus")]
use crate::auth::apply_auth_rate_limit;
use crate::auth::{extract_oci_bearer_token, unauthorized_with_www_authenticate};
use crate::response::oci_cors_headers;

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
/// Returns 200 OK if authenticated and the registry implements the OCI Distribution Spec.
/// Returns 401 Unauthorized with WWW-Authenticate header if not authenticated.
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

    // Try to extract and validate bearer token
    let token = extract_oci_bearer_token(&rqctx)
        .map_err(|_| unauthorized_with_www_authenticate(&rqctx, None))?;

    // Validate the OCI token
    let claims = context
        .token_key
        .validate_oci(&token)
        .map_err(|_| unauthorized_with_www_authenticate(&rqctx, None))?;

    // Apply rate limiting
    #[cfg(feature = "plus")]
    apply_auth_rate_limit(&rqctx.log, context, &claims).await?;

    // Return 200 OK with empty JSON body
    oci_cors_headers(
        Response::builder()
            .status(http::StatusCode::OK)
            .header(http::header::CONTENT_TYPE, "application/json"),
        &[http::Method::GET],
    )
    .body(Body::from("{}"))
    .map_err(|e| HttpError::for_internal_error(format!("Failed to build response: {e}")))
}
