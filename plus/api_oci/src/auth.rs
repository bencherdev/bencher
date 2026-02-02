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
        project::QueryProject,
        user::{QueryUser, auth::AuthUser, public::PublicUser},
    },
    public_conn,
};
use bencher_token::OciClaims;
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

/// Result of push access validation
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
#[expect(
    clippy::map_err_ignore,
    reason = "Intentionally discarding auth/validation errors for security"
)]
#[expect(
    clippy::too_many_lines,
    reason = "Auth logic needs to handle multiple cases"
)]
pub async fn validate_push_access(
    log: &Logger,
    rqctx: &RequestContext<ApiContext>,
    repository: &ProjectResourceId,
) -> Result<PushAccess, HttpError> {
    let context = rqctx.context();
    let repository_str = repository.to_string();
    let scope = format!("repository:{repository_str}:push");

    // Try to extract bearer token (optional for unclaimed projects)
    let token_result = extract_oci_bearer_token(rqctx);

    // Build a PublicUser based on authentication status
    let (public_user, claims) =
        build_public_user(log, context, rqctx, &token_result, &repository_str).await?;

    // Apply rate limiting based on authentication status
    #[cfg(feature = "plus")]
    match &public_user {
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

    // Check if the project exists
    if let Ok(query_project) = QueryProject::from_resource_id(public_conn!(context), repository) {
        // Project exists - check if its organization is claimed
        let query_organization =
            query_project
                .organization(public_conn!(context))
                .map_err(|e| {
                    HttpError::for_internal_error(format!("Failed to get organization: {e}"))
                })?;

        let is_claimed = query_organization
            .is_claimed(public_conn!(context))
            .map_err(|e| {
                HttpError::for_internal_error(format!("Failed to check claimed status: {e}"))
            })?;

        if is_claimed {
            // Organization is claimed - authentication is required
            let claims =
                claims.ok_or_else(|| unauthorized_with_www_authenticate(rqctx, Some(&scope)))?;

            slog::debug!(
                log,
                "Validating push access for claimed project";
                "project" => %query_project.uuid,
                "organization" => %query_organization.uuid
            );

            // Verify RBAC permission on the project
            if let PublicUser::Auth(auth_user) = &public_user {
                query_project
                    .try_allowed(&context.rbac, auth_user, Permission::Create)
                    .map_err(|_| {
                        HttpError::for_client_error(
                            None,
                            ClientErrorStatusCode::FORBIDDEN,
                            format!(
                                "Access denied to repository: {repository_str}. You need Create permission.",
                            ),
                        )
                    })?;
            } else {
                // This shouldn't happen since we checked for claims above
                return Err(unauthorized_with_www_authenticate(rqctx, Some(&scope)));
            }

            return Ok(PushAccess {
                project: query_project,
                claims: Some(claims),
            });
        }

        // Organization is unclaimed - allow unauthenticated push
        slog::info!(
            log,
            "Allowing push to unclaimed project";
            "project" => %query_project.uuid,
            "organization" => %query_organization.uuid,
            "authenticated" => claims.is_some()
        );

        // If authenticated user is pushing to unclaimed project, claim the organization.
        // This must succeed - if it fails, we reject the push to prevent the user
        // from thinking they own the project when they don't.
        if let PublicUser::Auth(auth_user) = &public_user {
            query_organization
                .claim(context, &auth_user.user)
                .await
                .map_err(|e| {
                    slog::error!(
                        log,
                        "Failed to claim organization during OCI push - rejecting push";
                        "organization" => %query_organization.uuid,
                        "user" => %auth_user.user.uuid,
                        "error" => %e
                    );
                    HttpError::for_internal_error(format!(
                        "Failed to claim organization: {e}. Push rejected to prevent security issues."
                    ))
                })?;
        }

        return Ok(PushAccess {
            project: query_project,
            claims,
        });
    }

    // Project doesn't exist - behavior depends on whether UUID or slug was used
    match repository.clone() {
        ProjectResourceId::Uuid(uuid) => {
            // UUIDs must reference existing projects
            slog::debug!(
                log,
                "OCI push to non-existent project by UUID";
                "uuid" => %uuid
            );
            Err(HttpError::for_client_error(
                None,
                ClientErrorStatusCode::NOT_FOUND,
                format!("Project not found: {uuid}"),
            ))
        },
        ProjectResourceId::Slug(slug) => {
            // Slugs trigger auto-creation
            slog::info!(
                log,
                "Creating project on-the-fly for OCI push";
                "slug" => %slug,
                "authenticated" => claims.is_some()
            );

            // Use the slug as the project name (converted to ResourceName)
            let slug_str: &str = slug.as_ref();
            let project_name: ResourceName = slug_str
                .parse()
                .map_err(|e| HttpError::for_internal_error(format!("Invalid project name: {e}")))?;

            let query_project =
                QueryProject::get_or_create(log, context, &public_user, repository, || {
                    Ok(project_name)
                })
                .await?;

            Ok(PushAccess {
                project: query_project,
                claims,
            })
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
    token_result: &Result<Jwt, HttpError>,
    repository_str: &str,
) -> Result<(PublicUser, Option<OciClaims>), HttpError> {
    if let Ok(token) = token_result
        && let Ok(claims) = validate_oci_access(context, token, repository_str, "push")
    {
        // Valid OCI token - load the user
        let email = claims.email().clone();
        if let Ok(query_user) = QueryUser::get_with_email(public_conn!(context), &email)
            && let Ok(auth_user) = AuthUser::load(public_conn!(context), query_user)
        {
            slog::debug!(
                log,
                "OCI push with valid authentication";
                "user" => %auth_user.user.uuid
            );
            return Ok((PublicUser::Auth(Box::new(auth_user)), Some(claims)));
        }
    }

    // No valid token or user not found - treat as public user
    // Extract remote IP for rate limiting
    let remote_ip = RateLimiting::remote_ip(log, rqctx.request.headers());
    slog::debug!(log, "OCI push without authentication (public user)"; "remote_ip" => ?remote_ip);
    Ok((PublicUser::Public(remote_ip), None))
}
