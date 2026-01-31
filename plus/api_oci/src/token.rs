//! OCI Token Endpoint
//!
//! - GET /v2/token - Exchange credentials for an OCI bearer token
//!
//! This endpoint implements the Docker Registry Auth specification.
//! Clients authenticate using Basic auth with their Bencher API token
//! as the password, and receive a short-lived JWT for OCI operations.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use bencher_endpoint::{CorsResponse, Endpoint, Get};
use bencher_json::{Email, Jwt};
use bencher_schema::context::ApiContext;
use bencher_token::OCI_TOKEN_TTL;
use chrono::Utc;
use dropshot::{Body, ClientErrorStatusCode, HttpError, Query, RequestContext, endpoint};
use http::Response;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::auth::unauthorized_with_www_authenticate;

/// Query parameters for token endpoint
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TokenQuery {
    /// Service identifier (e.g., "registry.bencher.dev")
    /// Currently unused but included for OCI spec compliance
    #[expect(dead_code)]
    pub service: Option<String>,
    /// Scope in format "repository:name:action,action"
    /// e.g., "repository:org/project:pull,push"
    pub scope: Option<String>,
}

/// Token response following Docker Registry Auth spec
#[derive(Debug, Serialize, JsonSchema)]
pub struct TokenResponse {
    /// The short-lived OCI JWT
    pub token: String,
    /// Token lifetime in seconds
    pub expires_in: u32,
    /// When the token was issued (RFC3339)
    pub issued_at: String,
}

/// CORS preflight for token endpoint
#[endpoint {
    method = OPTIONS,
    path = "/v0/auth/oci/token",
    tags = ["oci"],
}]
pub async fn oci_token_options(
    _rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

/// OCI token endpoint
///
/// Authenticates users via Basic auth (email:bencher-api-token)
/// and returns a short-lived JWT for OCI operations.
#[expect(
    clippy::map_err_ignore,
    reason = "Intentionally discarding API key validation error for security"
)]
#[endpoint {
    method = GET,
    path = "/v0/auth/oci/token",
    tags = ["oci"],
}]
pub async fn oci_token(
    rqctx: RequestContext<ApiContext>,
    query: Query<TokenQuery>,
) -> Result<Response<Body>, HttpError> {
    let context = rqctx.context();
    let query = query.into_inner();

    // 1. Extract Basic auth from Authorization header
    let (email, api_token) = extract_basic_auth(&rqctx)?;

    // 2. Validate the API token using existing validate_api_key
    let claims = context
        .token_key
        .validate_api_key(&api_token)
        .map_err(|_| unauthorized_with_www_authenticate(&rqctx, query.scope.as_deref()))?;

    // Verify the email matches the token subject
    if claims.email() != &email {
        return Err(unauthorized_with_www_authenticate(
            &rqctx,
            query.scope.as_deref(),
        ));
    }

    // 3. Parse scope to extract repository and actions
    let (repository, actions) = if let Some(scope) = &query.scope {
        parse_scope(scope)?
    } else {
        // No scope requested - token valid for base endpoint only
        (None, vec![])
    };

    // 4. Create OCI token with the validated scope
    // Note: We're not doing full RBAC validation here - that happens when the token is used.
    // The token just records what was requested; the actual permission check happens
    // on each API call when the token is presented.
    let jwt = context
        .token_key
        .new_oci(email, repository, actions)
        .map_err(|e| HttpError::for_internal_error(format!("Failed to create OCI token: {e}")))?;

    // 5. Build response
    let response = TokenResponse {
        token: jwt.to_string(),
        expires_in: OCI_TOKEN_TTL,
        issued_at: Utc::now().to_rfc3339(),
    };

    let body = serde_json::to_vec(&response)
        .map_err(|e| HttpError::for_internal_error(format!("Failed to serialize response: {e}")))?;

    Response::builder()
        .status(http::StatusCode::OK)
        .header(http::header::CONTENT_TYPE, "application/json")
        .header(http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(Body::from(body))
        .map_err(|e| HttpError::for_internal_error(format!("Failed to build response: {e}")))
}

/// Extract email and API token from Basic auth header
#[expect(
    clippy::map_err_ignore,
    reason = "Intentionally discarding decode errors for security"
)]
fn extract_basic_auth(rqctx: &RequestContext<ApiContext>) -> Result<(Email, Jwt), HttpError> {
    let headers = rqctx.request.headers();

    let auth_header = headers.get(http::header::AUTHORIZATION).ok_or_else(|| {
        unauthorized_with_www_authenticate(rqctx, None)
    })?;

    let auth_str = auth_header.to_str().map_err(|_| {
        HttpError::for_client_error(
            None,
            ClientErrorStatusCode::BAD_REQUEST,
            "Invalid Authorization header encoding".to_owned(),
        )
    })?;

    let (scheme, credentials) = auth_str.split_once(' ').ok_or_else(|| {
        unauthorized_with_www_authenticate(rqctx, None)
    })?;

    if scheme != "Basic" {
        return Err(unauthorized_with_www_authenticate(rqctx, None));
    }

    // Decode base64 credentials
    let decoded = STANDARD
        .decode(credentials)
        .map_err(|_| {
            HttpError::for_client_error(
                None,
                ClientErrorStatusCode::BAD_REQUEST,
                "Invalid base64 encoding in Authorization header".to_owned(),
            )
        })?;

    let decoded_str = String::from_utf8(decoded).map_err(|_| {
        HttpError::for_client_error(
            None,
            ClientErrorStatusCode::BAD_REQUEST,
            "Invalid UTF-8 in Authorization credentials".to_owned(),
        )
    })?;

    let (username, password) = decoded_str.split_once(':').ok_or_else(|| {
        HttpError::for_client_error(
            None,
            ClientErrorStatusCode::BAD_REQUEST,
            "Invalid Basic auth format (expected username:password)".to_owned(),
        )
    })?;

    let email: Email = username.parse().map_err(|_| {
        HttpError::for_client_error(
            None,
            ClientErrorStatusCode::BAD_REQUEST,
            "Invalid email format in username".to_owned(),
        )
    })?;

    let api_token: Jwt = password.parse().map_err(|_| {
        HttpError::for_client_error(
            None,
            ClientErrorStatusCode::BAD_REQUEST,
            "Invalid API token format in password".to_owned(),
        )
    })?;

    Ok((email, api_token))
}

/// Parse OCI scope string into repository and actions
///
/// Format: "repository:name:actions" where actions is comma-separated
/// Example: "repository:org/project:pull,push"
fn parse_scope(scope: &str) -> Result<(Option<String>, Vec<String>), HttpError> {
    let parts: Vec<&str> = scope.split(':').collect();

    if parts.len() != 3 {
        return Err(HttpError::for_client_error(
            None,
            ClientErrorStatusCode::BAD_REQUEST,
            format!("Invalid scope format: {scope}. Expected: repository:name:actions"),
        ));
    }

    // Safe to use get() since we checked length above
    let Some(resource_type) = parts.first() else {
        return Err(HttpError::for_client_error(
            None,
            ClientErrorStatusCode::BAD_REQUEST,
            format!("Invalid scope format: {scope}"),
        ));
    };
    let Some(repository_name) = parts.get(1) else {
        return Err(HttpError::for_client_error(
            None,
            ClientErrorStatusCode::BAD_REQUEST,
            format!("Invalid scope format: {scope}"),
        ));
    };
    let Some(actions_str) = parts.get(2) else {
        return Err(HttpError::for_client_error(
            None,
            ClientErrorStatusCode::BAD_REQUEST,
            format!("Invalid scope format: {scope}"),
        ));
    };

    if *resource_type != "repository" {
        return Err(HttpError::for_client_error(
            None,
            ClientErrorStatusCode::BAD_REQUEST,
            format!("Unsupported scope type: {resource_type}. Only 'repository' is supported."),
        ));
    }

    let actions: Vec<String> = actions_str
        .split(',')
        .map(|a| a.trim().to_owned())
        .filter(|a| !a.is_empty())
        .collect();

    // Validate actions
    for action in &actions {
        if action != "pull" && action != "push" {
            return Err(HttpError::for_client_error(
                None,
                ClientErrorStatusCode::BAD_REQUEST,
                format!("Unknown action: {action}. Supported: pull, push"),
            ));
        }
    }

    Ok((Some((*repository_name).to_owned()), actions))
}
