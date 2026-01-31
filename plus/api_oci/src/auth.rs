//! OCI Authentication Helpers
//!
//! Provides helper functions for OCI Distribution Spec authentication:
//! - WWW-Authenticate header generation for 401 responses
//! - Bearer token extraction from Authorization headers
//! - OCI token validation

use bencher_json::Jwt;
use bencher_schema::context::ApiContext;
use bencher_token::OciClaims;
use dropshot::{ClientErrorStatusCode, HttpError, RequestContext};

// Re-export from api_auth
pub use api_auth::oci::unauthorized_with_www_authenticate;

/// Extract OCI bearer token from Authorization header
///
/// Expects format: `Authorization: Bearer <token>`
pub fn extract_oci_bearer_token(
    rqctx: &RequestContext<ApiContext>,
) -> Result<Jwt, HttpError> {
    let headers = rqctx.request.headers();

    let auth_header = headers
        .get(http::header::AUTHORIZATION)
        .ok_or_else(|| {
            HttpError::for_client_error(
                None,
                ClientErrorStatusCode::UNAUTHORIZED,
                "Missing Authorization header".to_owned(),
            )
        })?;

    let auth_str = auth_header.to_str().map_err(|_err| {
        HttpError::for_client_error(
            None,
            ClientErrorStatusCode::BAD_REQUEST,
            "Invalid Authorization header encoding".to_owned(),
        )
    })?;

    let (scheme, token) = auth_str.split_once(' ').ok_or_else(|| {
        HttpError::for_client_error(
            None,
            ClientErrorStatusCode::BAD_REQUEST,
            "Invalid Authorization header format".to_owned(),
        )
    })?;

    if scheme != "Bearer" {
        return Err(HttpError::for_client_error(
            None,
            ClientErrorStatusCode::UNAUTHORIZED,
            "Expected Bearer authentication".to_owned(),
        ));
    }

    token.trim().parse().map_err(|e| {
        HttpError::for_client_error(
            None,
            ClientErrorStatusCode::BAD_REQUEST,
            format!("Invalid token format: {e}"),
        )
    })
}

/// Validate OCI token and check it grants access to the specified repository and action
#[expect(
    clippy::map_err_ignore,
    reason = "Intentionally discarding token validation error for security"
)]
pub fn validate_oci_access(
    context: &ApiContext,
    token: &Jwt,
    repository: &str,
    required_action: &str,
) -> Result<OciClaims, HttpError> {
    let claims = context.token_key.validate_oci(token).map_err(|_| {
        HttpError::for_client_error(
            None,
            ClientErrorStatusCode::UNAUTHORIZED,
            "Invalid or expired token".to_owned(),
        )
    })?;

    // Check repository matches (if token has a specific repository)
    if let Some(token_repo) = &claims.oci.repository
        && token_repo != repository
    {
        return Err(HttpError::for_client_error(
            None,
            ClientErrorStatusCode::FORBIDDEN,
            format!("Token not valid for repository: {repository}"),
        ));
    }

    // Check action is allowed
    let action_allowed = claims.oci.actions.iter().any(|a| a == required_action);
    if !action_allowed {
        return Err(HttpError::for_client_error(
            None,
            ClientErrorStatusCode::FORBIDDEN,
            format!("Token does not permit {required_action} action"),
        ));
    }

    Ok(claims)
}
