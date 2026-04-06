//! OCI Authentication Helpers
//!
//! Provides helper functions for OCI Distribution Spec authentication:
//! - WWW-Authenticate header generation for 401 responses
//! - Bearer token extraction from Authorization headers
//! - OCI token validation
//! - Push access validation with claimed/unclaimed project support and auto-creation
//! - Rate limiting for OCI requests

use bencher_json::oci::{
    OCI_ERROR_DENIED, OCI_ERROR_NAME_UNKNOWN, OCI_ERROR_UNAUTHORIZED, OCI_ERROR_UNSUPPORTED,
    oci_error_body,
};
use bencher_json::{Jwt, ProjectResourceId, ResourceName};
use bencher_rbac::project::Permission;
use bencher_schema::{
    context::{ApiContext, RateLimiting},
    model::{
        organization::{OrganizationId, QueryOrganization},
        project::QueryProject,
        user::{QueryUser, auth::AuthUser, public::PublicUser},
    },
    public_conn,
};
use bencher_token::{AuthOciClaims, OciAction, OciScopeClaims, RunnerOciClaims};
use dropshot::{ClientErrorStatusCode, HttpError, RequestContext};
use slog::Logger;

/// Identity that can pull from the OCI registry.
///
/// Pull endpoints accept public OCI tokens (anonymous),
/// user OCI tokens (sub: `Email`), and runner OCI tokens (sub: `RunnerUuid`).
pub enum OciPullIdentity {
    Public,
    Auth(AuthOciClaims),
    Runner(RunnerOciClaims),
}

// Re-export from api_auth
pub use api_auth::oci::unauthorized_with_www_authenticate;

/// Extract OCI bearer token from Authorization header
///
/// Expects format: `Authorization: Bearer <token>`
pub fn extract_oci_bearer_token(rqctx: &RequestContext<ApiContext>) -> Result<Jwt, HttpError> {
    let headers = rqctx.request.headers();

    let auth_header = headers.get(bencher_json::AUTHORIZATION).ok_or_else(|| {
        HttpError::for_client_error(
            None,
            ClientErrorStatusCode::UNAUTHORIZED,
            oci_error_body(OCI_ERROR_UNAUTHORIZED, "Missing Authorization header"),
        )
    })?;

    let auth_str = auth_header.to_str().map_err(|_err| {
        HttpError::for_client_error(
            None,
            ClientErrorStatusCode::BAD_REQUEST,
            oci_error_body(
                OCI_ERROR_UNSUPPORTED,
                "Invalid Authorization header encoding",
            ),
        )
    })?;

    let Some(token) = bencher_json::strip_bearer_token(auth_str) else {
        return Err(HttpError::for_client_error(
            None,
            ClientErrorStatusCode::UNAUTHORIZED,
            oci_error_body(OCI_ERROR_UNAUTHORIZED, "Expected Bearer authentication"),
        ));
    };

    token.parse().map_err(|_err| {
        HttpError::for_client_error(
            None,
            ClientErrorStatusCode::BAD_REQUEST,
            oci_error_body(OCI_ERROR_UNSUPPORTED, "Invalid token format"),
        )
    })
}

/// Validate that OCI scope claims grant access to the specified repository and action.
///
/// Shared scope-checking logic for all OCI token types (auth, runner, public).
fn validate_oci_scope(
    oci: &OciScopeClaims,
    repository: &str,
    required_action: &OciAction,
) -> Result<(), HttpError> {
    // Check repository matches (if token has a specific repository)
    if let Some(token_repo) = &oci.repository
        && token_repo != repository
    {
        return Err(HttpError::for_client_error(
            None,
            ClientErrorStatusCode::FORBIDDEN,
            oci_error_body(
                OCI_ERROR_DENIED,
                &format!("Token not valid for repository: {repository}"),
            ),
        ));
    }

    // Check action is allowed
    if !oci.actions.contains(required_action) {
        return Err(HttpError::for_client_error(
            None,
            ClientErrorStatusCode::FORBIDDEN,
            oci_error_body(
                OCI_ERROR_DENIED,
                &format!("Token does not permit {required_action:?} action"),
            ),
        ));
    }

    Ok(())
}

/// Try to validate a bearer token as a runner, user, or public OCI token,
/// then check that it grants pull access to the specified repository.
///
/// Runner tokens are tried first because in production the vast majority of OCI pull
/// requests come from runners. Only server admins pull otherwise, so we optimise for
/// the common case. Public tokens are tried last.
fn validate_pull_identity(
    context: &ApiContext,
    token: &Jwt,
    repository: &str,
) -> Result<OciPullIdentity, HttpError> {
    // Try runner token first (common case)
    if let Ok(runner_claims) = context.token_key.validate_oci_runner(token) {
        validate_oci_scope(&runner_claims.oci, repository, &OciAction::Pull)?;
        return Ok(OciPullIdentity::Runner(runner_claims));
    }

    // Try user token
    if let Ok(user_claims) = context.token_key.validate_oci_auth(token) {
        validate_oci_scope(&user_claims.oci, repository, &OciAction::Pull)?;
        return Ok(OciPullIdentity::Auth(user_claims));
    }

    // Try public token
    if let Ok(public_claims) = context.token_key.validate_oci_public(token) {
        validate_oci_scope(&public_claims.oci, repository, &OciAction::Pull)?;
        return Ok(OciPullIdentity::Public);
    }

    Err(HttpError::for_client_error(
        None,
        ClientErrorStatusCode::UNAUTHORIZED,
        oci_error_body(OCI_ERROR_UNAUTHORIZED, "Invalid or expired token"),
    ))
}

/// Validate pull access for an OCI operation.
///
/// Requires a Bearer token (runner, auth, or public). Public tokens are restricted
/// to unclaimed projects; runner and auth tokens have normal pull access.
///
/// Use this for read operations during push flows (e.g. HEAD blob checks).
#[expect(
    clippy::map_err_ignore,
    reason = "Intentionally discarding auth errors for security"
)]
pub async fn validate_pull_access(
    rqctx: &RequestContext<ApiContext>,
    repository: &ProjectResourceId,
) -> Result<QueryProject, HttpError> {
    let context = rqctx.context();
    let repository_str = repository.to_string();
    let scope = format!("repository:{repository_str}:pull");

    let token = extract_oci_bearer_token(rqctx)
        .map_err(|_| unauthorized_with_www_authenticate(rqctx, Some(&scope)))?;
    let identity = validate_pull_identity(context, &token, &repository_str)?;

    // Public tokens can only pull from unclaimed projects
    if matches!(identity, OciPullIdentity::Public) {
        let conn = public_conn!(context);
        if let Ok(project) = QueryProject::from_resource_id(conn, repository) {
            let is_claimed = project
                .organization(conn)
                .map_err(|e| {
                    HttpError::for_internal_error(format!("Failed to get organization: {e}"))
                })?
                .is_claimed(conn)
                .map_err(|e| {
                    HttpError::for_internal_error(format!("Failed to check claimed status: {e}"))
                })?;
            if is_claimed {
                return Err(unauthorized_with_www_authenticate(rqctx, Some(&scope)));
            }
        }
    }

    apply_pull_rate_limit(rqctx, &identity).await?;

    resolve_project(context, repository).await
}

/// Require push access for an OCI operation (simple ops like delete, not project creation).
///
/// Validates the bearer token as an authenticated user OCI token and checks push scope.
/// Use this for simple write operations that don't need the full project creation flow.
/// For operations that may create projects, use `validate_push_access` instead.
#[expect(
    clippy::map_err_ignore,
    reason = "Intentionally discarding auth errors for security"
)]
pub async fn require_push_access(
    rqctx: &RequestContext<ApiContext>,
    repository: &str,
) -> Result<(), HttpError> {
    let context = rqctx.context();
    let scope = format!("repository:{repository}:push");
    let token = extract_oci_bearer_token(rqctx)
        .map_err(|_| unauthorized_with_www_authenticate(rqctx, Some(&scope)))?;
    let claims = context
        .token_key
        .validate_oci_auth(&token)
        .map_err(|_err| {
            HttpError::for_client_error(
                None,
                ClientErrorStatusCode::UNAUTHORIZED,
                oci_error_body(OCI_ERROR_UNAUTHORIZED, "Invalid or expired token"),
            )
        })?;
    validate_oci_scope(&claims.oci, repository, &OciAction::Push)?;

    apply_user_rate_limit(&rqctx.log, context, &claims).await?;

    Ok(())
}

/// Result of push access validation (for operations that may create projects)
pub struct PushAccess {
    /// The project being pushed to (existing or newly created)
    pub project: QueryProject,
    /// OCI claims if authenticated, None if unauthenticated push to unclaimed project
    #[expect(dead_code, reason = "May be used in the future for audit logging")]
    pub claims: Option<AuthOciClaims>,
}

/// Validate push access for OCI operations and get or create the project
///
/// This function implements the claimed/unclaimed project logic with auto-creation:
/// - If the project exists and is claimed → requires auth token with Create permission
/// - If the project exists and is unclaimed → allows public or auth tokens
/// - If the project doesn't exist:
///   - If UUID is used → returns `NOT_FOUND` error (UUIDs must reference existing projects)
///   - If slug is used → creates the project (under user's org if authenticated, new unclaimed org if public)
///
/// Requires a Bearer token (auth or public). Returns the project and optional claims,
/// or an error if access is denied.
#[expect(
    clippy::map_err_ignore,
    reason = "Intentionally discarding auth errors for security"
)]
pub async fn validate_push_access(
    log: &Logger,
    rqctx: &RequestContext<ApiContext>,
    repository: &ProjectResourceId,
) -> Result<PushAccess, HttpError> {
    let context = rqctx.context();
    let repository_str = repository.to_string();
    let scope = format!("repository:{repository_str}:push");

    // Bearer token is required (Docker obtains one from the token endpoint)
    let token = extract_oci_bearer_token(rqctx)
        .map_err(|_| unauthorized_with_www_authenticate(rqctx, Some(&scope)))?;
    let (public_user, claims) =
        build_public_user(log, context, rqctx, token, &repository_str).await?;

    // Apply rate limiting based on authentication status
    apply_push_rate_limit(log, context, &public_user)?;

    // Try to find existing project, or create if using a slug
    let conn = public_conn!(context);
    match QueryProject::from_resource_id(conn, repository) {
        Ok(project) => {
            handle_existing_project(log, rqctx, context, project, &public_user, claims).await
        },
        Err(_) => handle_nonexistent_project(log, context, repository, &public_user, claims).await,
    }
}

/// Apply rate limiting for push operations based on authentication status.
///
/// OCI pushes use the general request rate limiter rather than the run rate
/// limiter so that multi-layer Docker pushes (which hit this for each blob
/// upload start and manifest PUT) don't exhaust the run quota before the
/// actual `bencher run` request.
fn apply_push_rate_limit(
    log: &Logger,
    context: &ApiContext,
    public_user: &PublicUser,
) -> Result<(), HttpError> {
    match public_user {
        PublicUser::Public(remote_ip) => {
            if let Some(remote_ip) = remote_ip {
                slog::debug!(log, "Applying public OCI push rate limit"; "remote_ip" => ?remote_ip);
                context.rate_limiting.public_request(*remote_ip)?;
            }
        },
        PublicUser::Auth(auth_user) => {
            slog::debug!(log, "Applying claimed OCI push rate limit"; "user_uuid" => %auth_user.user.uuid);
            context.rate_limiting.user_request(auth_user.user.uuid)?;
        },
    }
    Ok(())
}

/// Handle push access for an existing project
///
/// Checks if the organization is claimed or unclaimed and enforces appropriate access control.
async fn handle_existing_project(
    log: &Logger,
    rqctx: &RequestContext<ApiContext>,
    context: &ApiContext,
    project: QueryProject,
    public_user: &PublicUser,
    claims: Option<AuthOciClaims>,
) -> Result<PushAccess, HttpError> {
    let conn = public_conn!(context);
    let organization = project
        .organization(conn)
        .map_err(|e| HttpError::for_internal_error(format!("Failed to get organization: {e}")))?;

    let is_claimed = organization.is_claimed(conn).map_err(|e| {
        HttpError::for_internal_error(format!("Failed to check claimed status: {e}"))
    })?;

    if is_claimed {
        handle_claimed_project(log, rqctx, context, project, public_user, claims)
    } else {
        handle_unclaimed_project(log, context, project, organization, public_user, claims).await
    }
}

/// Handle push to a claimed project (requires authentication and RBAC permission)
#[expect(
    clippy::map_err_ignore,
    reason = "Intentionally discarding RBAC error details for security"
)]
fn handle_claimed_project(
    log: &Logger,
    rqctx: &RequestContext<ApiContext>,
    context: &ApiContext,
    project: QueryProject,
    public_user: &PublicUser,
    claims: Option<AuthOciClaims>,
) -> Result<PushAccess, HttpError> {
    let scope = format!("repository:{}:push", project.slug);

    // Authentication is required for claimed projects
    let claims = claims.ok_or_else(|| unauthorized_with_www_authenticate(rqctx, Some(&scope)))?;

    let PublicUser::Auth(auth_user) = public_user else {
        // This shouldn't happen since we have claims
        return Err(unauthorized_with_www_authenticate(rqctx, Some(&scope)));
    };

    slog::debug!(
        log,
        "Validating push access for claimed project";
        "project" => %project.uuid
    );

    // Verify RBAC permission
    project
        .try_allowed(&context.rbac, auth_user, Permission::Create)
        .map_err(|_| {
            HttpError::for_client_error(
                None,
                ClientErrorStatusCode::FORBIDDEN,
                oci_error_body(OCI_ERROR_DENIED, "Insufficient permissions"),
            )
        })?;

    Ok(PushAccess {
        project,
        claims: Some(claims),
    })
}

/// Handle push to an unclaimed project (allows unauthenticated, auto-claims if authenticated)
async fn handle_unclaimed_project(
    log: &Logger,
    context: &ApiContext,
    project: QueryProject,
    organization: QueryOrganization,
    public_user: &PublicUser,
    claims: Option<AuthOciClaims>,
) -> Result<PushAccess, HttpError> {
    slog::info!(
        log,
        "Allowing push to unclaimed project";
        "project" => %project.uuid,
        "organization" => %organization.uuid,
        "authenticated" => claims.is_some()
    );

    // If authenticated, claim the organization for the user
    if let PublicUser::Auth(auth_user) = public_user {
        organization
            .claim(log, context, &auth_user.user)
            .await
            .map_err(|e| {
                slog::error!(
                    log,
                    "Failed to claim organization during OCI push - rejecting push";
                    "organization" => %organization.uuid,
                    "user" => %auth_user.user.uuid,
                    "error" => %e
                );
                HttpError::for_internal_error(format!(
                    "Failed to claim organization: {e}. Push rejected to prevent security issues."
                ))
            })?;
    }

    Ok(PushAccess { project, claims })
}

/// Handle push to a non-existent project (UUID → 404, slug → auto-create)
async fn handle_nonexistent_project(
    log: &Logger,
    context: &ApiContext,
    repository: &ProjectResourceId,
    public_user: &PublicUser,
    claims: Option<AuthOciClaims>,
) -> Result<PushAccess, HttpError> {
    match repository {
        ProjectResourceId::Uuid(uuid) => {
            slog::debug!(log, "OCI push to non-existent project by UUID"; "uuid" => %uuid);
            Err(HttpError::for_client_error(
                None,
                ClientErrorStatusCode::NOT_FOUND,
                oci_error_body(
                    OCI_ERROR_NAME_UNKNOWN,
                    &format!("Repository not found: {uuid}"),
                ),
            ))
        },
        ProjectResourceId::Slug(slug) => {
            slog::info!(
                log,
                "Creating project on-the-fly for OCI push";
                "slug" => %slug,
                "authenticated" => claims.is_some()
            );

            let slug_str: &str = slug.as_ref();
            let project_name: ResourceName = slug_str.parse().map_err(|e| {
                HttpError::for_client_error(
                    None,
                    ClientErrorStatusCode::BAD_REQUEST,
                    format!("Invalid project name: {e}"),
                )
            })?;

            let project =
                QueryProject::get_or_create(log, context, public_user, repository, || {
                    Ok(project_name)
                })
                .await?;

            Ok(PushAccess { project, claims })
        },
    }
}

/// Apply rate limiting for OCI pull requests based on identity type.
///
/// - `Public` identity: applies IP-based public rate limiting.
/// - `Auth` identity: looks up the user by email and applies `user_request` rate limiting.
/// - `Runner` identity: skips rate limiting (runners already have per-runner rate limiting
///   on the claim endpoint).
async fn apply_pull_rate_limit(
    rqctx: &RequestContext<ApiContext>,
    identity: &OciPullIdentity,
) -> Result<(), HttpError> {
    let log = &rqctx.log;
    let context = rqctx.context();
    match identity {
        OciPullIdentity::Public => apply_public_rate_limit(log, context, rqctx),
        OciPullIdentity::Auth(claims) => apply_user_rate_limit(log, context, claims).await,
        OciPullIdentity::Runner(claims) => {
            slog::debug!(log, "Skipping rate limit for runner OCI pull"; "runner_uuid" => %claims.sub);
            Ok(())
        },
    }
}

/// Apply rate limiting for a user OCI token
///
/// Looks up the user by email and applies `user_request` rate limiting.
/// Used by push endpoints and the base endpoint which only accept user tokens.
pub(crate) async fn apply_user_rate_limit(
    log: &Logger,
    context: &ApiContext,
    claims: &AuthOciClaims,
) -> Result<(), HttpError> {
    let query_user =
        QueryUser::get_with_email(public_conn!(context), claims.email()).map_err(|_err| {
            HttpError::for_client_error(
                None,
                ClientErrorStatusCode::UNAUTHORIZED,
                oci_error_body(OCI_ERROR_UNAUTHORIZED, "Invalid or expired token"),
            )
        })?;
    slog::debug!(log, "Applying OCI request rate limit"; "user_uuid" => %query_user.uuid);
    context.rate_limiting.user_request(query_user.uuid)?;
    Ok(())
}

/// Apply rate limiting for unauthenticated OCI requests (upload sessions)
///
/// Uses `public_request` rate limiting based on the client's IP address.
/// This is used for upload session operations where the session ID serves as authentication.
pub fn apply_public_rate_limit(
    log: &Logger,
    context: &ApiContext,
    rqctx: &RequestContext<ApiContext>,
) -> Result<(), HttpError> {
    if let Some(remote_ip) = RateLimiting::remote_ip(log, rqctx.request.headers()) {
        slog::debug!(log, "Applying OCI public request rate limit"; "remote_ip" => ?remote_ip);
        context.rate_limiting.public_request(remote_ip)?;
    }
    Ok(())
}

/// Resolve a `ProjectResourceId` (UUID or slug) to a `QueryProject`
///
/// This performs a database lookup to find the project and return it.
/// Used by pull endpoints and upload session endpoints to get the project
/// for storage paths and bandwidth tracking.
pub async fn resolve_project(
    context: &ApiContext,
    resource_id: &ProjectResourceId,
) -> Result<QueryProject, HttpError> {
    let conn = public_conn!(context);
    QueryProject::from_resource_id(conn, resource_id).map_err(|_e| {
        HttpError::for_client_error(
            None,
            ClientErrorStatusCode::NOT_FOUND,
            oci_error_body(
                OCI_ERROR_NAME_UNKNOWN,
                &format!("Repository not found: {resource_id}"),
            ),
        )
    })
}

/// Build a `PublicUser` from an OCI Bearer token.
///
/// Returns `PublicUser::Auth` for auth tokens (with claims), or `PublicUser::Public`
/// for public (anonymous) tokens. Returns 401 if the token is neither.
async fn build_public_user(
    log: &Logger,
    context: &ApiContext,
    rqctx: &RequestContext<ApiContext>,
    token: Jwt,
    repository_str: &str,
) -> Result<(PublicUser, Option<AuthOciClaims>), HttpError> {
    // If the token decodes as an auth OCI token, it IS an auth token — propagate
    // scope errors (403) rather than falling through to try as a public token.
    if let Ok(claims) = context.token_key.validate_oci_auth(&token) {
        validate_oci_scope(&claims.oci, repository_str, &OciAction::Push)?;
        let email = claims.email().clone();
        let conn = public_conn!(context);
        let query_user = QueryUser::get_with_email(conn, &email).map_err(|_err| {
            HttpError::for_client_error(
                None,
                ClientErrorStatusCode::UNAUTHORIZED,
                oci_error_body(OCI_ERROR_UNAUTHORIZED, "Invalid token"),
            )
        })?;
        let auth_user = AuthUser::load(conn, query_user).map_err(|_err| {
            HttpError::for_client_error(
                None,
                ClientErrorStatusCode::UNAUTHORIZED,
                oci_error_body(OCI_ERROR_UNAUTHORIZED, "Invalid token"),
            )
        })?;
        slog::debug!(
            log,
            "OCI push with valid authentication";
            "user" => %auth_user.user.uuid
        );
        return Ok((PublicUser::Auth(Box::new(auth_user)), Some(claims)));
    }

    // Try as public OCI token (anonymous push) — must validate scope too.
    if let Ok(public_claims) = context.token_key.validate_oci_public(&token) {
        validate_oci_scope(&public_claims.oci, repository_str, &OciAction::Push)?;
        let remote_ip = RateLimiting::remote_ip(log, rqctx.request.headers());
        slog::debug!(log, "OCI push with public token (anonymous)"; "remote_ip" => ?remote_ip);
        return Ok((PublicUser::Public(remote_ip), None));
    }

    // Token provided but invalid as both auth and public
    slog::warn!(log, "OCI push with invalid token");
    Err(HttpError::for_client_error(
        None,
        ClientErrorStatusCode::UNAUTHORIZED,
        oci_error_body(OCI_ERROR_UNAUTHORIZED, "Invalid or expired token"),
    ))
}

/// Check bandwidth limit for a project's organization. Call BEFORE data transfer.
///
/// Returns the organization ID for use in `record_oci_bandwidth` without a second DB lookup.
pub(crate) async fn check_oci_bandwidth(
    context: &ApiContext,
    project: &QueryProject,
) -> Result<OrganizationId, HttpError> {
    let conn = public_conn!(context);
    let organization = project.organization(conn)?;
    let priority = organization.oci_bandwidth_priority(conn, &context.licensor)?;
    context
        .rate_limiting
        .check_oci_bandwidth(organization.id, priority, &organization)?;
    Ok(organization.id)
}

/// Record bytes transferred. Call AFTER successful data transfer.
pub(crate) fn record_oci_bandwidth(context: &ApiContext, org_id: OrganizationId, bytes: u64) {
    context.rate_limiting.record_oci_bandwidth(org_id, bytes);
}
