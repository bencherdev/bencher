//! OCI Authentication Helpers
//!
//! Provides helper functions for OCI Distribution Spec authentication:
//! - WWW-Authenticate header generation for 401 responses
//! - Bearer token extraction from Authorization headers
//! - OCI token validation
//! - Push access validation with claimed/unclaimed project support and auto-creation
//! - Rate limiting for OCI requests

use bencher_json::{Jwt, ProjectResourceId, ResourceName};
use bencher_rbac::project::Permission;
use bencher_schema::{
    context::{ApiContext, RateLimiting},
    model::{
        organization::QueryOrganization,
        project::QueryProject,
        user::{QueryUser, auth::AuthUser, public::PublicUser},
    },
    public_conn,
};
use bencher_token::{OciAction, OciClaims};
use dropshot::{ClientErrorStatusCode, HttpError, RequestContext};
use slog::Logger;

// Re-export from api_auth
pub use api_auth::oci::unauthorized_with_www_authenticate;

/// Extract OCI bearer token from Authorization header
///
/// Expects format: `Authorization: Bearer <token>`
pub fn extract_oci_bearer_token(rqctx: &RequestContext<ApiContext>) -> Result<Jwt, HttpError> {
    let headers = rqctx.request.headers();

    let auth_header = headers.get(http::header::AUTHORIZATION).ok_or_else(|| {
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

    if !scheme.eq_ignore_ascii_case("bearer") {
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
pub fn validate_oci_access(
    context: &ApiContext,
    token: &Jwt,
    repository: &str,
    required_action: &OciAction,
) -> Result<OciClaims, HttpError> {
    let claims = context.token_key.validate_oci(token).map_err(|_err| {
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
    if !claims.oci.actions.contains(required_action) {
        return Err(HttpError::for_client_error(
            None,
            ClientErrorStatusCode::FORBIDDEN,
            format!("Token does not permit {required_action:?} action"),
        ));
    }

    Ok(claims)
}

/// Result of pull/push access validation (for simple operations that don't need project info)
pub struct OciAccess {
    /// The validated OCI claims
    #[expect(dead_code, reason = "May be used in the future for audit logging")]
    pub claims: OciClaims,
}

/// Require pull access for an OCI operation
///
/// Validates the bearer token and checks it grants pull access to the specified repository.
/// Use this for simple read operations that don't need project info.
#[expect(
    clippy::map_err_ignore,
    reason = "Intentionally discarding auth errors for security"
)]
pub async fn require_pull_access(
    rqctx: &RequestContext<ApiContext>,
    repository: &str,
) -> Result<OciAccess, HttpError> {
    let context = rqctx.context();
    let scope = format!("repository:{repository}:pull");
    let token = extract_oci_bearer_token(rqctx)
        .map_err(|_| unauthorized_with_www_authenticate(rqctx, Some(&scope)))?;
    let claims = validate_oci_access(context, &token, repository, &OciAction::Pull)
        .map_err(|_| unauthorized_with_www_authenticate(rqctx, Some(&scope)))?;

    // Apply rate limiting
    #[cfg(feature = "plus")]
    apply_auth_rate_limit(&rqctx.log, context, &claims).await?;

    Ok(OciAccess { claims })
}

/// Require push access for an OCI operation (simple ops like delete, not project creation)
///
/// Validates the bearer token and checks it grants push access to the specified repository.
/// Use this for simple write operations that don't need the full project creation flow.
/// For operations that may create projects, use `validate_push_access` instead.
#[expect(
    clippy::map_err_ignore,
    reason = "Intentionally discarding auth errors for security"
)]
pub async fn require_push_access(
    rqctx: &RequestContext<ApiContext>,
    repository: &str,
) -> Result<OciAccess, HttpError> {
    let context = rqctx.context();
    let scope = format!("repository:{repository}:push");
    let token = extract_oci_bearer_token(rqctx)
        .map_err(|_| unauthorized_with_www_authenticate(rqctx, Some(&scope)))?;
    let claims = validate_oci_access(context, &token, repository, &OciAction::Push)
        .map_err(|_| unauthorized_with_www_authenticate(rqctx, Some(&scope)))?;

    // Apply rate limiting
    #[cfg(feature = "plus")]
    apply_auth_rate_limit(&rqctx.log, context, &claims).await?;

    Ok(OciAccess { claims })
}

/// Result of push access validation (for operations that may create projects)
pub struct PushAccess {
    /// The project being pushed to (existing or newly created)
    pub project: QueryProject,
    /// OCI claims if authenticated, None if unauthenticated push to unclaimed project
    #[expect(dead_code, reason = "May be used in the future for audit logging")]
    pub claims: Option<OciClaims>,
}

/// Validate push access for OCI operations and get or create the project
///
/// This function implements the claimed/unclaimed project logic with auto-creation:
/// - If the project exists and is claimed → requires valid authentication with Create permission
/// - If the project exists and is unclaimed → allows unauthenticated push
/// - If the project doesn't exist:
///   - If UUID is used → returns `NOT_FOUND` error (UUIDs must reference existing projects)
///   - If slug is used → creates the project (under user's org if authenticated, new unclaimed org if not)
///
/// Returns the project and optional claims, or an error if access is denied.
pub async fn validate_push_access(
    log: &Logger,
    rqctx: &RequestContext<ApiContext>,
    repository: &ProjectResourceId,
) -> Result<PushAccess, HttpError> {
    let context = rqctx.context();
    let repository_str = repository.to_string();

    // Build authentication context (optional for unclaimed projects)
    let token = extract_oci_bearer_token(rqctx).ok();
    let (public_user, claims) =
        build_public_user(log, context, rqctx, token, &repository_str).await?;

    // Apply rate limiting based on authentication status
    #[cfg(feature = "plus")]
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

/// Apply rate limiting for push operations based on authentication status
#[cfg(feature = "plus")]
fn apply_push_rate_limit(
    log: &Logger,
    context: &ApiContext,
    public_user: &PublicUser,
) -> Result<(), HttpError> {
    match public_user {
        PublicUser::Public(remote_ip) => {
            if let Some(remote_ip) = remote_ip {
                slog::debug!(log, "Applying unclaimed OCI push rate limit"; "remote_ip" => ?remote_ip);
                context.rate_limiting.unclaimed_run(*remote_ip)?;
            }
        },
        PublicUser::Auth(auth_user) => {
            slog::debug!(log, "Applying claimed OCI push rate limit"; "user_uuid" => %auth_user.user.uuid);
            context.rate_limiting.claimed_run(auth_user.user.uuid)?;
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
    claims: Option<OciClaims>,
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
    claims: Option<OciClaims>,
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
                "Insufficient permissions".to_owned(),
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
    claims: Option<OciClaims>,
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
            .claim(context, &auth_user.user)
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
    claims: Option<OciClaims>,
) -> Result<PushAccess, HttpError> {
    match repository {
        ProjectResourceId::Uuid(uuid) => {
            slog::debug!(log, "OCI push to non-existent project by UUID"; "uuid" => %uuid);
            Err(HttpError::for_client_error(
                None,
                ClientErrorStatusCode::NOT_FOUND,
                format!("Project not found: {uuid}"),
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

/// Apply rate limiting for authenticated OCI requests
///
/// Uses `user_request` rate limiting for authenticated users based on their UUID.
/// This is used for pull operations (GET/HEAD) where the user has a valid OCI token.
#[cfg(feature = "plus")]
pub async fn apply_auth_rate_limit(
    log: &Logger,
    context: &ApiContext,
    claims: &OciClaims,
) -> Result<(), HttpError> {
    if let Ok(query_user) = QueryUser::get_with_email(public_conn!(context), claims.email()) {
        slog::debug!(log, "Applying OCI request rate limit"; "user_uuid" => %query_user.uuid);
        context.rate_limiting.user_request(query_user.uuid)?;
    }
    Ok(())
}

/// Apply rate limiting for unauthenticated OCI requests (upload sessions)
///
/// Uses `public_request` rate limiting based on the client's IP address.
/// This is used for upload session operations where the session ID serves as authentication.
#[cfg(feature = "plus")]
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

/// Build a `PublicUser` from OCI authentication
///
/// Returns (`PublicUser`, `Option<OciClaims>`) where claims is Some if authenticated.
/// For unauthenticated users, extracts the remote IP from headers for rate limiting.
async fn build_public_user(
    log: &Logger,
    context: &ApiContext,
    rqctx: &RequestContext<ApiContext>,
    token: Option<Jwt>,
    repository_str: &str,
) -> Result<(PublicUser, Option<OciClaims>), HttpError> {
    if let Some(token) = token {
        // Token was provided -- it MUST be valid; don't silently downgrade to public
        let claims = validate_oci_access(context, &token, repository_str, &OciAction::Push)
            .map_err(|e| {
                slog::warn!(log, "OCI push with invalid token"; "error" => %e);
                e
            })?;
        let email = claims.email().clone();
        let conn = public_conn!(context);
        let query_user = QueryUser::get_with_email(conn, &email).map_err(|_err| {
            HttpError::for_client_error(
                None,
                ClientErrorStatusCode::UNAUTHORIZED,
                "Invalid token".to_owned(),
            )
        })?;
        let auth_user = AuthUser::load(conn, query_user).map_err(|_err| {
            HttpError::for_client_error(
                None,
                ClientErrorStatusCode::UNAUTHORIZED,
                "Invalid token".to_owned(),
            )
        })?;
        slog::debug!(
            log,
            "OCI push with valid authentication";
            "user" => %auth_user.user.uuid
        );
        Ok((PublicUser::Auth(Box::new(auth_user)), Some(claims)))
    } else {
        // No token provided -- treat as public user (for unclaimed projects)
        let remote_ip = RateLimiting::remote_ip(log, rqctx.request.headers());
        slog::debug!(log, "OCI push without authentication (public user)"; "remote_ip" => ?remote_ip);
        Ok((PublicUser::Public(remote_ip), None))
    }
}
