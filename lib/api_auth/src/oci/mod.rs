#![cfg(feature = "plus")]

//! OCI Token Endpoint
//!
//! - GET /v0/auth/oci/token - Exchange credentials for an OCI bearer token
//! - POST /v0/auth/oci/token - `OAuth2` token exchange (Docker 29+ / containerd)
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
use bencher_endpoint::{CorsResponse, Endpoint, Get, Post};
use bencher_json::oci::{OCI_ERROR_DENIED, OCI_ERROR_UNAUTHORIZED, oci_error_body};
use bencher_json::{Email, Jwt, PROJECT_KEY_PREFIX, ProjectKey, ProjectKeyHash, ProjectResourceId};
use bencher_rbac::project::Permission;
use bencher_schema::{
    context::ApiContext,
    error::issue_error,
    model::{
        project::{QueryProject, key::QueryProjectKey},
        user::{QueryUser, auth::AuthUser},
    },
    public_conn,
};
use bencher_token::OciAction;
use chrono::Utc;
use dropshot::{
    Body, ClientErrorStatusCode, HttpError, Query, RequestContext, UntypedBody, endpoint,
};
use http::Response;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// OCI token TTL: 5 minutes (300 seconds)
pub const OCI_TOKEN_TTL: u32 = 300;

/// Query parameters for token endpoint
///
/// The `scope` parameter is parsed from the raw query string rather than
/// through Dropshot's `Query<T>`, because Docker 29+ (containerd image store)
/// sends multiple `scope` query params which `serde_urlencoded` rejects as
/// duplicate fields on a scalar type.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TokenQuery {
    /// Service identifier (e.g., "registry.bencher.dev")
    pub service: Option<String>,
}

/// Token response following Docker Registry Auth spec (GET)
#[derive(Debug, Serialize, JsonSchema)]
pub struct TokenResponse {
    /// The short-lived OCI JWT
    pub token: String,
    /// Token lifetime in seconds
    pub expires_in: u32,
    /// When the token was issued (RFC3339)
    pub issued_at: String,
}

/// `OAuth2` token form body for POST requests (containerd / Docker 29+)
#[derive(Debug, Deserialize)]
struct OAuthTokenForm {
    grant_type: String,
    #[expect(dead_code, reason = "accepted for spec compliance")]
    service: Option<String>,
    scope: Option<String>,
    #[expect(dead_code, reason = "accepted for spec compliance")]
    client_id: Option<String>,
    username: Option<String>,
    password: Option<String>,
    #[expect(dead_code, reason = "accepted for spec compliance")]
    refresh_token: Option<String>,
    #[expect(dead_code, reason = "accepted for spec compliance")]
    access_type: Option<String>,
}

/// `OAuth2` token response for POST requests
///
/// Per the `OAuth2` token spec, the response field is `access_token` (not `token`).
#[derive(Debug, Serialize)]
struct OAuthTokenResponse {
    access_token: String,
    expires_in: u32,
    scope: Option<String>,
    issued_at: String,
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
    Ok(Endpoint::cors(&[Get.into(), Post.into()]))
}

/// OCI token endpoint (GET)
///
/// Authenticates via Basic auth and returns a short-lived JWT for OCI operations.
/// Supports two credential types:
/// - `email:api-key-jwt`: user authentication
/// - `project-slug-or-uuid:bencher_run_xxxxx`: project key authentication
///
/// If no Basic auth credentials are provided, issues a public (anonymous) OCI token.
///
/// Accepts multiple `scope` query parameters (Docker 29+ / containerd sends these).
#[endpoint {
    method = GET,
    path = "/v0/auth/oci/token",
    tags = ["auth", "oci"],
}]
pub async fn auth_oci_token_get(
    rqctx: RequestContext<ApiContext>,
    _query: Query<TokenQuery>,
) -> Result<Response<Body>, HttpError> {
    let context = rqctx.context();

    // Docker 29+ (containerd) sends multiple `scope` query params that Dropshot
    // can't deserialize into a scalar. Parse all scopes from the raw query string.
    let scopes = extract_scopes_from_query(rqctx.request.uri());
    let (repository, actions) = if scopes.is_empty() {
        (None, vec![])
    } else {
        parse_scopes(&scopes)?
    };

    let scope_str = scopes.first().map(String::as_str);
    let credentials = extract_basic_credentials(&rqctx)?;
    let jwt =
        dispatch_oci_token(&rqctx, context, scope_str, repository, actions, credentials).await?;

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

/// Build an `OAuth2` token-endpoint error body per RFC 6749 §5.2.
///
/// The token endpoint speaks `OAuth2`, so its errors use `{"error", "error_description"}`
/// rather than the OCI registry's `oci_error_body` envelope.
fn oauth_error_body(error: &str, description: &str) -> String {
    serde_json::json!({
        "error": error,
        "error_description": description,
    })
    .to_string()
}

/// OCI token endpoint (POST) — `OAuth2` token exchange
///
/// Accepts `application/x-www-form-urlencoded` body with `grant_type=password`.
/// Docker 29+ (containerd image store) uses this flow before falling back to GET.
#[endpoint {
    method = POST,
    path = "/v0/auth/oci/token",
    tags = ["auth", "oci"],
}]
pub async fn auth_oci_token_post(
    rqctx: RequestContext<ApiContext>,
    body: UntypedBody,
) -> Result<Response<Body>, HttpError> {
    let context = rqctx.context();

    let form: OAuthTokenForm = serde_urlencoded::from_bytes(body.as_bytes()).map_err(|e| {
        HttpError::for_client_error(
            None,
            ClientErrorStatusCode::BAD_REQUEST,
            oauth_error_body("invalid_request", &format!("Invalid form body: {e}")),
        )
    })?;

    if form.grant_type == "refresh_token" {
        return Err(HttpError::for_client_error(
            None,
            ClientErrorStatusCode::BAD_REQUEST,
            oauth_error_body(
                "unsupported_grant_type",
                "Bencher does not issue refresh tokens",
            ),
        ));
    }

    if form.grant_type != "password" {
        return Err(HttpError::for_client_error(
            None,
            ClientErrorStatusCode::BAD_REQUEST,
            oauth_error_body(
                "unsupported_grant_type",
                &format!("expected \"password\", got \"{}\"", form.grant_type),
            ),
        ));
    }

    let credentials = match (form.username, form.password) {
        (Some(username), Some(password)) if !username.is_empty() => Some((username, password)),
        _ => extract_basic_credentials(&rqctx)?,
    };

    let Some(credentials) = credentials else {
        return Err(HttpError::for_client_error(
            None,
            ClientErrorStatusCode::BAD_REQUEST,
            oauth_error_body(
                "invalid_request",
                "grant_type=password requires username and password",
            ),
        ));
    };

    // Containerd joins multiple scopes with spaces in the POST body:
    // form.Set("scope", strings.Join(scopes, " "))
    let (repository, actions) = if let Some(scope) = &form.scope {
        let scopes: Vec<String> = scope
            .split(' ')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();
        parse_scopes(&scopes)?
    } else {
        (None, vec![])
    };

    let scope_str = form.scope.as_ref().and_then(|s| s.split(' ').next());
    let jwt = dispatch_oci_token(
        &rqctx,
        context,
        scope_str,
        repository,
        actions,
        Some(credentials),
    )
    .await?;

    let response = OAuthTokenResponse {
        access_token: jwt.to_string(),
        expires_in: OCI_TOKEN_TTL,
        scope: form.scope,
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

/// Dispatch to the appropriate token issuer based on credential type.
async fn dispatch_oci_token(
    rqctx: &RequestContext<ApiContext>,
    context: &ApiContext,
    scope: Option<&str>,
    repository: Option<String>,
    actions: Vec<OciAction>,
    credentials: Option<(String, String)>,
) -> Result<Jwt, HttpError> {
    match credentials {
        Some((username, password)) if password.starts_with(PROJECT_KEY_PREFIX) => {
            if let (Ok(project), Ok(project_key)) = (
                username.parse::<ProjectResourceId>(),
                password.parse::<ProjectKey>(),
            ) {
                project_key_oci_token(
                    rqctx,
                    context,
                    scope,
                    &project,
                    &project_key,
                    repository,
                    actions,
                )
                .await
            } else {
                Err(unauthorized_with_www_authenticate(rqctx, scope))
            }
        },
        Some((username, password)) => {
            if let (Ok(email), Ok(api_token)) = (username.parse::<Email>(), password.parse::<Jwt>())
            {
                auth_oci_token(
                    rqctx, context, scope, &email, &api_token, repository, actions,
                )
                .await
            } else {
                Err(unauthorized_with_www_authenticate(rqctx, scope))
            }
        },
        None => context
            .token_key
            .new_oci_public(OCI_TOKEN_TTL, repository, actions)
            .map_err(|e| {
                HttpError::for_internal_error(format!("Failed to create public OCI token: {e}"))
            }),
    }
}

/// Issue an authenticated OCI token after validating credentials and RBAC.
async fn auth_oci_token(
    rqctx: &RequestContext<ApiContext>,
    context: &ApiContext,
    scope: Option<&str>,
    email: &Email,
    api_token: &Jwt,
    repository: Option<String>,
    actions: Vec<OciAction>,
) -> Result<Jwt, HttpError> {
    // Validate the API token
    let claims = context
        .token_key
        .validate_api_key(api_token)
        .map_err(|e| log_unauthorized_with_www_authenticate(rqctx, scope, &e))?;

    // Verify the email matches the token subject
    if claims.email() != email {
        return Err(unauthorized_with_www_authenticate(rqctx, scope));
    }

    // Check admin status for pull-only requests
    // Only server admins can pull OCI images standalone (to prevent abuse of the registry).
    // When push is also requested, we keep pull because Docker's push protocol
    // requires pull access (HEAD blob checks) for the same repository.
    // The token is already repository-scoped, so pull is constrained to that single repo.
    if actions.contains(&OciAction::Pull) && !actions.contains(&OciAction::Push) {
        let conn = public_conn!(context);
        let query_user = QueryUser::get_with_email(conn, email)
            .map_err(|e| log_unauthorized_with_www_authenticate(rqctx, scope, &e))?;
        let auth_user = AuthUser::load(conn, query_user)
            .map_err(|e| log_unauthorized_with_www_authenticate(rqctx, scope, &e))?;

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
                .map_err(|e| {
                    HttpError::for_internal_error(format!("Failed to query organization: {e}"))
                })?
                .is_claimed(conn)
                .map_err(|e| {
                    HttpError::for_internal_error(format!(
                        "Failed to check organization claim status: {e}"
                    ))
                })?;

            if is_claimed {
                let query_user = QueryUser::get_with_email(conn, email)
                    .map_err(|e| log_unauthorized_with_www_authenticate(rqctx, scope, &e))?;
                let auth_user = AuthUser::load(conn, query_user)
                    .map_err(|e| log_unauthorized_with_www_authenticate(rqctx, scope, &e))?;

                query_project
                    .try_allowed(&context.rbac, &auth_user, Permission::Create)
                    .inspect_err(|e| {
                        slog::info!(&rqctx.log, "OCI RBAC check failed"; "error" => %e);
                    })
                    .map_err(|_err| {
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
        _ = write!(www_auth, ",scope=\"{sanitized_scope}\"");
    }

    let mut error = HttpError::for_client_error(
        None,
        ClientErrorStatusCode::UNAUTHORIZED,
        oci_error_body(OCI_ERROR_UNAUTHORIZED, "Authentication required"),
    );

    // Best-effort: header may fail if value is not valid ASCII
    _ = error.add_header(http::header::WWW_AUTHENTICATE, &www_auth);

    error
}

pub fn log_unauthorized_with_www_authenticate(
    rqctx: &RequestContext<ApiContext>,
    scope: Option<&str>,
    error: &dyn std::fmt::Display,
) -> HttpError {
    slog::info!(&rqctx.log, "OCI auth failed"; "error" => %error);
    unauthorized_with_www_authenticate(rqctx, scope)
}

/// Issue a project-scoped OCI token after validating a project key.
async fn project_key_oci_token(
    rqctx: &RequestContext<ApiContext>,
    context: &ApiContext,
    scope: Option<&str>,
    project_rid: &ProjectResourceId,
    project_key: &ProjectKey,
    repository: Option<String>,
    actions: Vec<OciAction>,
) -> Result<Jwt, HttpError> {
    // Pull-only requests are not allowed for project key auth.
    // Only bare metal runners should be pulling, and they use runner tokens.
    if actions.contains(&OciAction::Pull) && !actions.contains(&OciAction::Push) {
        return Err(HttpError::for_client_error(
            None,
            ClientErrorStatusCode::FORBIDDEN,
            oci_error_body(
                OCI_ERROR_DENIED,
                "Project keys cannot be used for pull-only access. Use a runner token to pull.",
            ),
        ));
    }

    let key_hash = ProjectKeyHash::from(project_key);
    let now = context.clock.now();

    let query_key = QueryProjectKey::from_hash(public_conn!(context), &key_hash, now)
        .map_err(|e| log_unauthorized_with_www_authenticate(rqctx, scope, &e))?;

    let query_project = QueryProject::get(public_conn!(context), query_key.project_id)
        .map_err(|e| log_unauthorized_with_www_authenticate(rqctx, scope, &e))?;

    if !query_project.matches_resource_id(project_rid) {
        return Err(unauthorized_with_www_authenticate(rqctx, scope));
    }

    // If scope specifies a repository, verify it matches the key's project
    if let Some(repo_name) = &repository
        && let Ok(repo_rid) = repo_name.parse::<ProjectResourceId>()
        && !query_project.matches_resource_id(&repo_rid)
    {
        return Err(HttpError::for_client_error(
            None,
            ClientErrorStatusCode::FORBIDDEN,
            oci_error_body(
                OCI_ERROR_DENIED,
                "Project key is not authorized for the requested repository",
            ),
        ));
    }

    context.rate_limiting.project_request(query_project.uuid)?;

    slog::info!(
        &rqctx.log,
        "Issuing OCI project token via project key";
        "project_key_uuid" => %query_key.uuid,
        "project_uuid" => %query_project.uuid
    );

    context
        .token_key
        .new_oci_project(query_project.uuid, OCI_TOKEN_TTL, repository, actions)
        .map_err(|e| {
            HttpError::for_internal_error(format!("Failed to create OCI project token: {e}"))
        })
}

/// Extract raw username and password from Basic auth header.
///
/// Returns `Ok(None)` if no Authorization header or non-Basic scheme (anonymous).
/// Returns `Ok(Some(...))` if valid Basic credentials were extracted.
/// Returns `Err(...)` if the header is Basic but the payload is malformed.
fn extract_basic_credentials(
    rqctx: &RequestContext<ApiContext>,
) -> Result<Option<(String, String)>, HttpError> {
    let headers = rqctx.request.headers();
    let Some(auth_header) = headers.get(http::header::AUTHORIZATION) else {
        return Ok(None);
    };
    let auth_str = auth_header
        .to_str()
        .inspect_err(|e| {
            slog::info!(&rqctx.log, "Invalid Authorization header encoding"; "error" => %e);
        })
        .map_err(|_err| unauthorized_with_www_authenticate(rqctx, None))?;
    let Some((scheme, credentials)) = auth_str.split_once(' ') else {
        return Ok(None);
    };
    if !scheme.eq_ignore_ascii_case("Basic") {
        return Ok(None);
    }
    let decoded = STANDARD
        .decode(credentials)
        .inspect_err(|e| {
            slog::info!(&rqctx.log, "Invalid base64 in Basic auth"; "error" => %e);
        })
        .map_err(|_err| unauthorized_with_www_authenticate(rqctx, None))?;
    let decoded_str = String::from_utf8(decoded)
        .inspect_err(|e| {
            slog::info!(&rqctx.log, "Invalid UTF-8 in Basic auth"; "error" => %e);
        })
        .map_err(|_err| unauthorized_with_www_authenticate(rqctx, None))?;
    let Some((username, password)) = decoded_str.split_once(':') else {
        slog::info!(&rqctx.log, "Missing colon in Basic auth credentials");
        return Err(unauthorized_with_www_authenticate(rqctx, None));
    };
    Ok(Some((username.to_owned(), password.to_owned())))
}

/// Parse multiple OCI scope strings into a single repository + its actions.
///
/// Docker 29+ (containerd) sends multiple `scope` query params, e.g.:
/// `scope=repository:proj:pull&scope=repository:proj:pull,push`
///
/// The issued token is single-repository, so we select one target repository
/// and union the actions requested for it:
/// - The target is the first scope that names a repository — the registry's
///   challenge scope, i.e. the resource the client is actually accessing.
/// - Actions are unioned **only** from scopes whose repository matches the
///   target. Cached scopes that containerd may append for *other* repositories
///   (or any repository-less scope) are dropped, so they can never widen the
///   token's actions or retarget it.
fn parse_scopes(scopes: &[String]) -> Result<(Option<String>, Vec<OciAction>), HttpError> {
    // Parse every scope first so a malformed scope still errors, as before.
    let parsed: Vec<(Option<String>, Vec<OciAction>)> = scopes
        .iter()
        .map(|s| parse_scope(s))
        .collect::<Result<_, _>>()?;

    let target = parsed.iter().find_map(|(repo, _)| repo.clone());

    let mut actions = Vec::new();
    for (repo, scope_actions) in parsed {
        if repo.as_deref() == target.as_deref() {
            for action in scope_actions {
                if !actions.contains(&action) {
                    actions.push(action);
                }
            }
        }
    }

    Ok((target, actions))
}

/// Parse a single OCI scope string into repository and actions.
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

/// Extract all `scope` query parameter values from a URI.
///
/// Docker 29+ (containerd) sends multiple `scope` query params, e.g.:
/// `?scope=repository:proj:pull&scope=repository:proj:pull,push`
///
/// Dropshot's `Query<T>` requires scalar types, so we parse the raw query string.
fn extract_scopes_from_query(uri: &http::Uri) -> Vec<String> {
    let Some(query) = uri.query() else {
        return Vec::new();
    };
    serde_urlencoded::from_str::<Vec<(String, String)>>(query)
        .unwrap_or_default()
        .into_iter()
        .filter(|(key, _)| key == "scope")
        .map(|(_, value)| value)
        .collect()
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
        parse_scope("invalid").unwrap_err();
    }

    #[test]
    fn parse_scope_invalid_resource_type() {
        parse_scope("image:myrepo:pull").unwrap_err();
    }

    #[test]
    fn parse_scope_invalid_action() {
        parse_scope("repository:myrepo:delete").unwrap_err();
    }

    #[test]
    fn parse_scope_sanitizes_quotes() {
        let (repo, actions) = parse_scope("repository:org/project:pull").unwrap();
        assert_eq!(repo, Some("org/project".to_owned()));
        assert_eq!(actions, vec![OciAction::Pull]);
    }

    #[test]
    fn parse_scopes_merges_actions() {
        let scopes = vec![
            "repository:the-computer:pull".to_owned(),
            "repository:the-computer:pull,push".to_owned(),
        ];
        let (repo, actions) = parse_scopes(&scopes).unwrap();
        assert_eq!(repo, Some("the-computer".to_owned()));
        assert_eq!(actions, vec![OciAction::Pull, OciAction::Push]);
    }

    #[test]
    fn parse_scopes_single() {
        let scopes = vec!["repository:myrepo:push".to_owned()];
        let (repo, actions) = parse_scopes(&scopes).unwrap();
        assert_eq!(repo, Some("myrepo".to_owned()));
        assert_eq!(actions, vec![OciAction::Push]);
    }

    #[test]
    fn parse_scopes_empty() {
        let scopes: Vec<String> = vec![];
        let (repo, actions) = parse_scopes(&scopes).unwrap();
        assert_eq!(repo, None);
        assert!(actions.is_empty());
    }

    #[test]
    fn parse_scopes_ignores_other_repos() {
        let scopes = vec![
            "repository:repo-a:pull".to_owned(),
            "repository:repo-b:push".to_owned(),
        ];
        let (repo, actions) = parse_scopes(&scopes).unwrap();
        assert_eq!(repo, Some("repo-a".to_owned()));
        assert_eq!(actions, vec![OciAction::Pull]);
    }

    // A foreign repository's actions must never leak into the target token,
    // even when they overlap/exceed the target's own actions.
    #[test]
    fn parse_scopes_excludes_other_repo_actions() {
        let scopes = vec![
            "repository:repo-a:pull".to_owned(),
            "repository:repo-b:pull,push".to_owned(),
        ];
        let (repo, actions) = parse_scopes(&scopes).unwrap();
        assert_eq!(repo, Some("repo-a".to_owned()));
        assert_eq!(actions, vec![OciAction::Pull]);
    }

    // Multiple scopes for the same (target) repository still union, in order.
    #[test]
    fn parse_scopes_unions_same_repo_in_order() {
        let scopes = vec![
            "repository:r:push".to_owned(),
            "repository:r:pull".to_owned(),
        ];
        let (repo, actions) = parse_scopes(&scopes).unwrap();
        assert_eq!(repo, Some("r".to_owned()));
        assert_eq!(actions, vec![OciAction::Push, OciAction::Pull]);
    }
}
