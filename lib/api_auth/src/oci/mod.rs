#![cfg(feature = "plus")]

//! OCI Token Endpoint
//!
//! - GET /v0/auth/oci/token - Exchange credentials for an OCI bearer token
//!
//! This endpoint implements the Docker Registry Auth specification.
//! Clients authenticate using Basic auth with their Bencher API token
//! as the password, and receive a short-lived JWT for OCI operations.
//!
//! Authorization:
//! - "pull" action requires server admin privileges
//! - "push" action requires Create permission on the project (for claimed orgs)

use base64::{Engine as _, engine::general_purpose::STANDARD};
use bencher_endpoint::{CorsResponse, Endpoint, Get};
use bencher_json::{Email, Jwt, ProjectResourceId};
use bencher_rbac::project::Permission;
use bencher_schema::{
    context::ApiContext,
    model::{
        project::QueryProject,
        user::{QueryUser, auth::AuthUser},
    },
    public_conn,
};
use chrono::Utc;
use dropshot::{Body, ClientErrorStatusCode, HttpError, Query, RequestContext, endpoint};
use http::Response;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// OCI token TTL: 5 minutes (300 seconds)
pub const OCI_TOKEN_TTL: u32 = 300;

/// Query parameters for token endpoint
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TokenQuery {
    /// Service identifier (e.g., "registry.bencher.dev")
    /// Not currently used but accepted for OCI spec compliance
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
    tags = ["auth", "oci"],
}]
pub async fn auth_oci_token_options(
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
    tags = ["auth", "oci"],
}]
pub async fn auth_oci_token_get(
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

    // 4. Check admin status for pull requests
    // Only server admins can pull OCI images (to prevent abuse of the registry)
    if actions.contains(&"pull".to_owned()) {
        let query_user = QueryUser::get_with_email(public_conn!(context), &email)
            .map_err(|_| unauthorized_with_www_authenticate(&rqctx, query.scope.as_deref()))?;
        let auth_user = AuthUser::load(public_conn!(context), query_user)
            .map_err(|_| unauthorized_with_www_authenticate(&rqctx, query.scope.as_deref()))?;

        if !auth_user.is_admin(&context.rbac) {
            return Err(HttpError::for_client_error(
                None,
                ClientErrorStatusCode::FORBIDDEN,
                "Only server admins can pull OCI images".to_owned(),
            ));
        }
    }

    // 5. Validate RBAC permissions for push if a repository is requested AND the organization is claimed
    // The repository name maps to a project slug
    // For unclaimed organizations, we allow push without RBAC
    if actions.contains(&"push".to_owned())
        && let Some(repo_name) = &repository
        // The repository name is a project UUID or slug
        && let Ok(project_id) = repo_name.parse::<ProjectResourceId>()
        && let Ok(query_project) =
            QueryProject::from_resource_id(public_conn!(context), &project_id)
    {
        // Check if the organization is claimed
        let is_claimed = if let Ok(org) = query_project.organization(public_conn!(context)) {
            org.is_claimed(public_conn!(context)).unwrap_or(false)
        } else {
            false
        };

        // Only require RBAC permissions if the organization is claimed
        if is_claimed {
            // Load the user to check permissions
            let query_user = QueryUser::get_with_email(public_conn!(context), &email)
                .map_err(|_| unauthorized_with_www_authenticate(&rqctx, query.scope.as_deref()))?;
            let auth_user = AuthUser::load(public_conn!(context), query_user)
                .map_err(|_| unauthorized_with_www_authenticate(&rqctx, query.scope.as_deref()))?;

            // Check if user has Create permission for push
            query_project
                .try_allowed(&context.rbac, &auth_user, Permission::Create)
                .map_err(|_| {
                    HttpError::for_client_error(
                        None,
                        ClientErrorStatusCode::FORBIDDEN,
                        format!(
                            "Access denied to repository: {repo_name}. You need Create permission to push.",
                        ),
                    )
                })?;
        }
        // If organization is unclaimed, skip RBAC check and issue token
    }
    // If project doesn't exist, we still issue the token.
    // The actual operation will fail with a proper error when they try to use it.

    // 6. Create OCI token with the validated scope
    let jwt = context
        .token_key
        .new_oci(email, OCI_TOKEN_TTL, repository, actions)
        .map_err(|e| HttpError::for_internal_error(format!("Failed to create OCI token: {e}")))?;

    // 7. Build response
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

/// Create a 401 Unauthorized error with WWW-Authenticate header
///
/// Per the OCI Distribution Spec, when authentication is required,
/// the registry returns 401 with a WWW-Authenticate header indicating
/// how to obtain a token.
pub fn unauthorized_with_www_authenticate(
    rqctx: &RequestContext<ApiContext>,
    scope: Option<&str>,
) -> HttpError {
    use std::fmt::Write as _;

    let context = rqctx.context();

    // Build the realm URL from the request's scheme and host
    // The token endpoint is at /v0/auth/oci/token
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

/// Extract email and API token from Basic auth header
#[expect(
    clippy::map_err_ignore,
    reason = "Intentionally discarding decode errors for security"
)]
fn extract_basic_auth(rqctx: &RequestContext<ApiContext>) -> Result<(Email, Jwt), HttpError> {
    let headers = rqctx.request.headers();

    let auth_header = headers
        .get(http::header::AUTHORIZATION)
        .ok_or_else(|| unauthorized_with_www_authenticate(rqctx, None))?;

    let auth_str = auth_header.to_str().map_err(|_| {
        HttpError::for_client_error(
            None,
            ClientErrorStatusCode::BAD_REQUEST,
            "Invalid Authorization header encoding".to_owned(),
        )
    })?;

    let (scheme, credentials) = auth_str
        .split_once(' ')
        .ok_or_else(|| unauthorized_with_www_authenticate(rqctx, None))?;

    if scheme != "Basic" {
        return Err(unauthorized_with_www_authenticate(rqctx, None));
    }

    // Decode base64 credentials
    let decoded = STANDARD.decode(credentials).map_err(|_| {
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

    let [resource_type, repository_name, actions_str] = parts.as_slice() else {
        return Err(HttpError::for_client_error(
            None,
            ClientErrorStatusCode::BAD_REQUEST,
            format!("Invalid scope format: {scope}. Expected: repository:name:actions"),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_scope_valid() {
        let (repo, actions) = parse_scope("repository:org/project:pull,push").unwrap();
        assert_eq!(repo, Some("org/project".to_owned()));
        assert_eq!(actions, vec!["pull", "push"]);
    }

    #[test]
    fn test_parse_scope_single_action() {
        let (repo, actions) = parse_scope("repository:myrepo:pull").unwrap();
        assert_eq!(repo, Some("myrepo".to_owned()));
        assert_eq!(actions, vec!["pull"]);
    }

    #[test]
    fn test_parse_scope_invalid_format() {
        let result = parse_scope("invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_scope_invalid_resource_type() {
        let result = parse_scope("image:myrepo:pull");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_scope_invalid_action() {
        let result = parse_scope("repository:myrepo:delete");
        assert!(result.is_err());
    }
}
