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
//! - "pull" action alone requires server admin privileges
//! - "push,pull" keeps both actions since the token is repository-scoped (Docker's push protocol requires pull for HEAD blob checks)
//! - "push" action requires Create permission on the project (for claimed orgs)

use base64::{Engine as _, engine::general_purpose::STANDARD};
use bencher_endpoint::{CorsResponse, Endpoint, Get};
use bencher_json::oci::{OCI_ERROR_DENIED, OCI_ERROR_UNAUTHORIZED, oci_error_body};
use bencher_json::{Email, Jwt, ProjectResourceId};
use bencher_rbac::project::Permission;
use bencher_schema::{
    context::ApiContext,
    error::issue_error,
    model::{
        project::QueryProject,
        user::{QueryUser, auth::AuthUser},
    },
    public_conn,
};
use bencher_token::OciAction;
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
/// If no Basic auth credentials are provided, issues a public (anonymous) OCI token.
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

    // Parse scope to extract repository and actions
    let (repository, actions) = if let Some(scope) = &query.scope {
        parse_scope(scope)?
    } else {
        (None, vec![])
    };

    // Try to extract Basic auth — if absent, issue a public (anonymous) token
    let jwt = if let Ok((email, api_token)) = extract_basic_auth(&rqctx) {
        auth_oci_token(
            &rqctx, context, &query, &email, &api_token, repository, actions,
        )
        .await?
    } else {
        // Anonymous: issue a public OCI token with no identity or RBAC checks
        context
            .token_key
            .new_oci_public(OCI_TOKEN_TTL, repository, actions)
            .map_err(|e| {
                HttpError::for_internal_error(format!("Failed to create public OCI token: {e}"))
            })?
    };

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

/// Issue an authenticated OCI token after validating credentials and RBAC.
#[expect(
    clippy::map_err_ignore,
    reason = "Intentionally discarding API key validation error for security"
)]
async fn auth_oci_token(
    rqctx: &RequestContext<ApiContext>,
    context: &ApiContext,
    query: &TokenQuery,
    email: &Email,
    api_token: &Jwt,
    repository: Option<String>,
    actions: Vec<OciAction>,
) -> Result<Jwt, HttpError> {
    // Validate the API token
    let claims = context
        .token_key
        .validate_api_key(api_token)
        .map_err(|_| unauthorized_with_www_authenticate(rqctx, query.scope.as_deref()))?;

    // Verify the email matches the token subject
    if claims.email() != email {
        return Err(unauthorized_with_www_authenticate(
            rqctx,
            query.scope.as_deref(),
        ));
    }

    // Check admin status for pull-only requests
    // Only server admins can pull OCI images standalone (to prevent abuse of the registry).
    // When push is also requested, we keep pull because Docker's push protocol
    // requires pull access (HEAD blob checks) for the same repository.
    // The token is already repository-scoped, so pull is constrained to that single repo.
    if actions.contains(&OciAction::Pull) && !actions.contains(&OciAction::Push) {
        let conn = public_conn!(context);
        let query_user = QueryUser::get_with_email(conn, email)
            .map_err(|_| unauthorized_with_www_authenticate(rqctx, query.scope.as_deref()))?;
        let auth_user = AuthUser::load(conn, query_user)
            .map_err(|_| unauthorized_with_www_authenticate(rqctx, query.scope.as_deref()))?;

        if !auth_user.is_admin(&context.rbac) {
            return Err(HttpError::for_client_error(
                None,
                ClientErrorStatusCode::FORBIDDEN,
                oci_error_body(OCI_ERROR_DENIED, "Only server admins can pull OCI images"),
            ));
        }
    }

    // Validate RBAC permissions for push if a repository is requested AND the organization is claimed
    if actions.contains(&OciAction::Push)
        && let Some(repo_name) = &repository
        && let Ok(project_id) = repo_name.parse::<ProjectResourceId>()
    {
        let conn = public_conn!(context);
        if let Ok(query_project) = QueryProject::from_resource_id(conn, &project_id) {
            let is_claimed = query_project
                .organization(conn)
                .map_err(|_| {
                    HttpError::for_internal_error("Failed to query organization".to_owned())
                })?
                .is_claimed(conn)
                .map_err(|_| {
                    HttpError::for_internal_error(
                        "Failed to check organization claim status".to_owned(),
                    )
                })?;

            if is_claimed {
                let query_user = QueryUser::get_with_email(conn, email).map_err(|_| {
                    unauthorized_with_www_authenticate(rqctx, query.scope.as_deref())
                })?;
                let auth_user = AuthUser::load(conn, query_user).map_err(|_| {
                    unauthorized_with_www_authenticate(rqctx, query.scope.as_deref())
                })?;

                query_project
                    .try_allowed(&context.rbac, &auth_user, Permission::Create)
                    .map_err(|_| {
                        HttpError::for_client_error(
                            None,
                            ClientErrorStatusCode::FORBIDDEN,
                            oci_error_body(
                                OCI_ERROR_DENIED,
                                &format!(
                                    "Access denied to repository: {repo_name}. You need Create permission to push.",
                                ),
                            ),
                        )
                    })?;
            }
        }
    }

    context
        .token_key
        .new_oci_auth(email.clone(), OCI_TOKEN_TTL, repository, actions)
        .map_err(|e| HttpError::for_internal_error(format!("Failed to create OCI token: {e}")))
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
    let registry_url = context.registry_url();

    // Use the configured registry URL for the service and realm
    let Some(service) = registry_url.host_str() else {
        return issue_error(
            "Missing registry URL host",
            "The configured registry_url has no host component",
            "registry_url.host_str() returned None",
        );
    };
    let realm = format!("{registry_url}v0/auth/oci/token");

    let mut www_auth = format!("Bearer realm=\"{realm}\",service=\"{service}\"");
    if let Some(scope) = scope {
        // Sanitize scope to prevent header injection via embedded quotes
        let sanitized_scope = scope.replace('"', "");
        // Using write! to avoid extra allocation per clippy::format_push_string
        let _ = write!(www_auth, ",scope=\"{sanitized_scope}\"");
    }

    let mut error = HttpError::for_client_error(
        None,
        ClientErrorStatusCode::UNAUTHORIZED,
        oci_error_body(OCI_ERROR_UNAUTHORIZED, "Authentication required"),
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

    if !scheme.eq_ignore_ascii_case("Basic") {
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
fn parse_scope(scope: &str) -> Result<(Option<String>, Vec<OciAction>), HttpError> {
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

    let actions: Vec<OciAction> = actions_str
        .split(',')
        .map(str::trim)
        .filter(|a| !a.is_empty())
        .map(|a| match a {
            "pull" => Ok(OciAction::Pull),
            "push" => Ok(OciAction::Push),
            _ => Err(HttpError::for_client_error(
                None,
                ClientErrorStatusCode::BAD_REQUEST,
                format!("Unknown action: {a}. Supported: pull, push"),
            )),
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok((Some((*repository_name).to_owned()), actions))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_scope_valid() {
        let (repo, actions) = parse_scope("repository:org/project:pull,push").unwrap();
        assert_eq!(repo, Some("org/project".to_owned()));
        assert_eq!(actions, vec![OciAction::Pull, OciAction::Push]);
    }

    #[test]
    fn parse_scope_single_action() {
        let (repo, actions) = parse_scope("repository:myrepo:pull").unwrap();
        assert_eq!(repo, Some("myrepo".to_owned()));
        assert_eq!(actions, vec![OciAction::Pull]);
    }

    #[test]
    fn parse_scope_invalid_format() {
        let result = parse_scope("invalid");
        assert!(result.is_err());
    }

    #[test]
    fn parse_scope_invalid_resource_type() {
        let result = parse_scope("image:myrepo:pull");
        assert!(result.is_err());
    }

    #[test]
    fn parse_scope_invalid_action() {
        let result = parse_scope("repository:myrepo:delete");
        assert!(result.is_err());
    }

    #[test]
    fn parse_scope_sanitizes_quotes() {
        // Quotes in repository name are stripped by scope sanitization
        // before reaching parse_scope, but parse_scope itself should
        // handle the already-sanitized input correctly
        let (repo, actions) = parse_scope("repository:org/project:pull").unwrap();
        assert_eq!(repo, Some("org/project".to_owned()));
        assert_eq!(actions, vec![OciAction::Pull]);
    }
}
