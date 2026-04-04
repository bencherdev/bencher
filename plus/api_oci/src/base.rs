//! OCI Base Endpoint - GET /v2/
//!
//! Returns 200 OK to indicate this registry implements the OCI Distribution Spec.
//! If a valid Bearer token is provided, rate limiting is applied.
//! This is the first endpoint clients call to verify registry compatibility.

use bencher_endpoint::{CorsResponse, Endpoint, Get};
use bencher_schema::context::ApiContext;
use dropshot::{Body, HttpError, RequestContext, endpoint};
use http::Response;

#[cfg(feature = "plus")]
use crate::auth::{apply_public_rate_limit, apply_user_rate_limit, extract_oci_bearer_token};
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
/// Returns 200 OK to indicate this registry implements the OCI Distribution Spec.
/// If a valid Bearer token is provided, user-level rate limiting is applied.
/// If no token is provided, public IP-based rate limiting is applied.
/// Actual authorization is enforced at the individual push/pull endpoints.
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

    // Apply rate limiting based on authentication status
    #[cfg(feature = "plus")]
    if let Ok(token) = extract_oci_bearer_token(&rqctx) {
        // Token was provided — it MUST be valid; don't silently downgrade to public
        let claims = context
            .token_key
            .validate_oci(&token)
            .map_err(|_| crate::auth::unauthorized_with_www_authenticate(&rqctx, None))?;
        apply_user_rate_limit(&rqctx.log, context, &claims).await?;
    } else {
        apply_public_rate_limit(&rqctx.log, context, &rqctx)?;
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
