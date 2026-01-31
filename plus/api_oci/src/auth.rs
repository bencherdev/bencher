//! OCI Authentication Helpers
//!
//! Provides helper functions for OCI Distribution Spec authentication:
//! - WWW-Authenticate header generation for 401 responses
//! - Bearer token extraction from Authorization headers
//! - OCI token validation

use std::fmt::Write as _;

use bencher_json::Jwt;
use bencher_schema::context::ApiContext;
use bencher_token::OciClaims;
use dropshot::{ClientErrorStatusCode, HttpError, RequestContext};

/// Create a 401 Unauthorized error with WWW-Authenticate header
///
/// Per the OCI Distribution Spec, when authentication is required,
/// the registry returns 401 with a WWW-Authenticate header indicating
/// how to obtain a token.
pub fn unauthorized_with_www_authenticate(
    rqctx: &RequestContext<ApiContext>,
    scope: Option<&str>,
) -> HttpError {
    let context = rqctx.context();

    // Build the realm URL from the request's scheme and host
    // The token endpoint is at /v0/auth/oci/token to avoid router conflicts with /v2/{name}
    let scheme = if rqctx.request.uri().scheme_str() == Some("https") {
        "https"
    } else {
        "http"
    };
    let host = rqctx
        .request
        .headers()
        .get(http::header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("localhost");
    let realm = format!("{scheme}://{host}/v0/auth/oci/token");

    let service = context
        .console_url
        .host_str()
        .unwrap_or("registry.bencher.dev");

    let mut www_auth = format!("Bearer realm=\"{realm}\",service=\"{service}\"");
    if let Some(scope) = scope {
        // Using write! to avoid extra allocation per clippy::format_push_string
        let _ = write!(www_auth, ",scope=\"{scope}\"");
    }

    let mut error = HttpError::for_client_error(
        None,
        ClientErrorStatusCode::UNAUTHORIZED,
        "Authentication required".to_owned(),
    );

    // Add WWW-Authenticate header - ignore error if it fails
    let _ = error.add_header(http::header::WWW_AUTHENTICATE, &www_auth);

    error
}

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
