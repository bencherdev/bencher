use std::ops::Deref;

use async_trait::async_trait;
use bencher_json::{Jwt, Sanitize, UserKey, UserKeyHash};
#[cfg(feature = "plus")]
use bencher_json::{OAuthScopes, system::payment::JsonCustomer};
use bencher_rbac::{
    Organization, Project, Server, User as RbacUser,
    server::Permission,
    user::{OrganizationRoles, ProjectRoles},
};
use bencher_token::Audience;
use diesel::{ExpressionMethods as _, OptionalExtension as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::{
    ApiEndpointBodyContentType, ExtensionMode, ExtractorMetadata, HttpError, RequestContext,
    ServerContext, SharedExtractor,
};
use oso::{PolarValue, ToPolar};

#[cfg(feature = "plus")]
use crate::context::OAuthServer;
use crate::{
    context::{ApiContext, DbConnection, Rbac},
    error::{BEARER_TOKEN_FORMAT, bad_request_error, issue_error, unauthorized_error},
    model::{
        organization::OrganizationId,
        project::ProjectId,
        user::{
            key::{QueryUserKey, UserKeyId},
            token::QueryToken,
        },
    },
    public_conn, schema,
};

use super::QueryUser;
#[cfg(feature = "plus")]
use super::oauth::{authenticate, is_oauth_credential};

const INVALID_USER_KEY: &str = "Invalid user key";

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user: QueryUser,
    pub organizations: Vec<OrganizationId>,
    pub projects: Vec<OrgProjectId>,
    pub rbac: RbacUser,
    pub credential: AuthCredential,
}

impl AuthUser {
    // This is required due to a limitation in `dropshot` where only four extractors are allowed.
    pub async fn new(rqctx: &RequestContext<ApiContext>) -> Result<Self, HttpError> {
        let bearer_token = BearerToken::from_request(rqctx).await?;
        Self::from_token(rqctx.context(), bearer_token).await
    }

    pub async fn from_token(
        context: &ApiContext,
        bearer_token: BearerToken,
    ) -> Result<Self, HttpError> {
        // Check out a single connection for the entire auth flow:
        // credential resolution and RBAC hydration share one pool checkout.
        let conn = public_conn!(context);
        let (query_user, credential) = match bearer_token {
            BearerToken::Jwt(jwt) => Self::user_from_jwt(context, conn, &jwt)?,
            BearerToken::UserKey(key) => {
                let (query_user, query_key) = Self::user_from_user_key(context, conn, &key)?;
                (query_user, AuthCredential::UserKey(query_key.id))
            },
        };

        #[cfg(feature = "plus")]
        context.rate_limiting.user_request(query_user.uuid)?;

        Self::load_inner(conn, query_user, credential)
    }

    fn user_from_jwt(
        context: &ApiContext,
        conn: &mut DbConnection,
        jwt: &Jwt,
    ) -> Result<(QueryUser, AuthCredential), HttpError> {
        let claims = context
            .token_key
            .validate_client(jwt)
            .map_err(|e| bad_request_error(format!("Failed to validate JSON Web Token: {e}")))?;
        let email = claims.email();

        let query_user = QueryUser::get_with_email(conn, email)?;
        query_user.check_is_locked()?;

        // API keys created via `POST /tokens` are persisted in the `token` table and
        // can be revoked. Reject any JWT whose row has `revoked` set. A JWT with no
        // row (e.g. one issued before revocation tracking existed, or any
        // externally-signed test fixture) is accepted — we only block tokens we know
        // have been revoked. Short-lived client (browser session) JWTs are never
        // persisted and skip this lookup.
        if claims.audience() == Audience::ApiKey.as_str()
            && let Some(row) = QueryToken::get_by_user_jwt(conn, query_user.id, jwt)
                .optional()
                .map_err(|e| unauthorized_error(format!("Failed to look up API token: {e}")))?
            && row.revoked.is_some()
        {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserTokenRevokedUse);
            return Err(unauthorized_error(
                "This API token has been revoked and is no longer valid",
            ));
        }

        let credential = if claims.audience() == Audience::Client.as_str() {
            AuthCredential::Session
        } else {
            AuthCredential::ApiToken
        };
        Ok((query_user, credential))
    }

    /// Authenticate via a `bencher_user_*` API key.
    /// Performs a single indexed lookup that returns the key row alongside its owning
    /// user. `revoked`, `expiration`, and `user.locked` are checked in Rust so each
    /// failure mode can be reported distinctly via `UserKeyAuthFailureReason` without
    /// a second query. All adverse cases return the same opaque `INVALID_USER_KEY`
    /// message to avoid leaking which condition tripped.
    /// Public so the OCI token endpoint (`api_auth`) can validate `docker login`
    /// user-key credentials through the exact same checks and telemetry.
    pub fn user_from_user_key(
        context: &ApiContext,
        conn: &mut DbConnection,
        key: &UserKey,
    ) -> Result<(QueryUser, QueryUserKey), HttpError> {
        let key_hash = UserKeyHash::from(key);
        let lookup = QueryUserKey::from_hash(conn, &key_hash)
            .optional()
            .inspect_err(|err| {
                issue_error(
                    "Failed to lookup user key",
                    &format!("Failed to lookup user key by hash: {key_hash}"),
                    err,
                );
            })
            .map_err(|_err| unauthorized_error(INVALID_USER_KEY))?;

        let Some((query_user, query_key)) = lookup else {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserKeyAuthFailed(
                bencher_otel::UserKeyAuthFailureReason::NotFound,
            ));
            return Err(unauthorized_error(INVALID_USER_KEY));
        };

        if query_key.revoked.is_some() {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserKeyAuthFailed(
                bencher_otel::UserKeyAuthFailureReason::Revoked,
            ));
            return Err(unauthorized_error(INVALID_USER_KEY));
        }

        if query_key.expiration.timestamp() <= context.clock.now().timestamp() {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserKeyAuthFailed(
                bencher_otel::UserKeyAuthFailureReason::Expired,
            ));
            return Err(unauthorized_error(INVALID_USER_KEY));
        }

        if query_user.locked {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserKeyAuthFailed(
                bencher_otel::UserKeyAuthFailureReason::Locked,
            ));
            return Err(unauthorized_error(INVALID_USER_KEY));
        }

        Ok((query_user, query_key))
    }

    /// Authenticate an OAuth access token presented to the MCP server, returning its scopes.
    /// The user acts without server admin authority, whatever their account holds.
    #[cfg(feature = "plus")]
    pub async fn from_oauth_token(
        context: &ApiContext,
        oauth: &OAuthServer,
        token: &str,
    ) -> Result<(Self, OAuthScopes), HttpError> {
        let conn = public_conn!(context);
        let (mut query_user, query_grant) = authenticate(context, conn, oauth, token)?;
        context.rate_limiting.user_request(query_user.uuid)?;
        query_user.admin = false;
        let credential = AuthCredential::OAuthGrant {
            user_key_id: query_grant.user_key_id,
        };
        let auth_user = Self::load_inner(conn, query_user, credential)?;
        Ok((auth_user, query_grant.scope))
    }

    pub fn reload(&self, conn: &mut DbConnection) -> Result<Self, HttpError> {
        Self::load_inner(conn, self.user.clone(), self.credential)
    }

    /// Load a user for an internal path, such as OCI, which is never a user session.
    pub fn load(conn: &mut DbConnection, query_user: QueryUser) -> Result<Self, HttpError> {
        Self::load_inner(conn, query_user, AuthCredential::ApiToken)
    }

    fn load_inner(
        conn: &mut DbConnection,
        query_user: QueryUser,
        credential: AuthCredential,
    ) -> Result<Self, HttpError> {
        let (org_ids, org_roles) = Self::organization_roles(conn, &query_user)?;
        let (proj_ids, proj_roles) = Self::project_roles(conn, &query_user)?;

        let rbac = RbacUser {
            admin: query_user.admin,
            locked: query_user.locked,
            organizations: org_roles,
            projects: proj_roles,
        };

        Ok(Self {
            user: query_user,
            organizations: org_ids,
            projects: proj_ids,
            rbac,
            credential,
        })
    }

    fn organization_roles(
        conn: &mut DbConnection,
        query_user: &QueryUser,
    ) -> Result<(Vec<OrganizationId>, OrganizationRoles), HttpError> {
        let roles = schema::organization_role::table
            .inner_join(schema::organization::table)
            .filter(schema::organization_role::user_id.eq(query_user.id))
            .filter(schema::organization::deleted.is_null())
            .order(schema::organization_role::organization_id)
            .select((
                schema::organization_role::organization_id,
                schema::organization_role::role,
            ))
            .load::<(OrganizationId, String)>(conn)
            .map_err(|e| {
                issue_error(
                    "User can't query organization roles",
                    &format!(
                        "My user ({email}) on Bencher failed to query organization roles.",
                        email = query_user.email
                    ),
                    e,
                )
            })?;

        let org_ids = roles.iter().map(|(org_id, _)| *org_id).collect();
        let roles = roles
            .into_iter()
            .filter_map(|(org_id, role)| match role.parse() {
                Ok(role) => Some((org_id.to_string(), role)),
                Err(e) => {
                    let _err = issue_error(
                        "Failed to parse organization role",
                        &format!("My user ({email}) on Bencher has an invalid organization role ({role}).", email = query_user.email),
                        e,
                    );
                    None
                },
            })
            .collect();

        Ok((org_ids, roles))
    }

    fn project_roles(
        conn: &mut DbConnection,
        query_user: &QueryUser,
    ) -> Result<(Vec<OrgProjectId>, ProjectRoles), HttpError> {
        let roles = schema::project_role::table
            .inner_join(schema::project::table)
            .filter(schema::project_role::user_id.eq(query_user.id))
            .filter(schema::project::deleted.is_null())
            .order(schema::project_role::project_id)
            .select((
                schema::project::organization_id,
                schema::project_role::project_id,
                schema::project_role::role,
            ))
            .load::<(OrganizationId, ProjectId, String)>(conn)
            .map_err(|e| {
                issue_error(
                    "User can't query project roles",
                    &format!(
                        "My user ({email}) on Bencher failed to query project roles.",
                        email = query_user.email
                    ),
                    e,
                )
            })?;

        let ids = roles
            .iter()
            .map(|(org_id, project_id, _)| OrgProjectId {
                org_id: *org_id,
                project_id: *project_id,
            })
            .collect();
        let roles = roles
            .into_iter()
            .filter_map(|(_, id, role)| match role.parse() {
                Ok(role) => Some((id.to_string(), role)),
                Err(e) => {
                    let _err = issue_error(
                        "Failed to parse project role",
                        &format!(
                            "My user ({email}) on Bencher has an invalid project role ({role}).",
                            email = query_user.email
                        ),
                        e,
                    );
                    None
                },
            })
            .collect();

        Ok((ids, roles))
    }

    pub fn is_admin(&self, rbac: &Rbac) -> bool {
        rbac.is_allowed_unwrap(self, Permission::Administer, Server)
    }

    /// Whether the request authenticated with a login session rather than a token or key.
    pub fn is_session(&self) -> bool {
        matches!(self.credential, AuthCredential::Session)
    }

    pub fn user_key_id(&self) -> Option<UserKeyId> {
        match self.credential {
            AuthCredential::UserKey(user_key_id) => Some(user_key_id),
            #[cfg(feature = "plus")]
            AuthCredential::OAuthGrant { user_key_id } => user_key_id,
            AuthCredential::Session | AuthCredential::ApiToken => None,
        }
    }

    pub fn organizations(
        &self,
        rbac: &Rbac,
        action: bencher_rbac::organization::Permission,
    ) -> Vec<OrganizationId> {
        self.organizations
            .iter()
            .filter_map(|org_id| {
                rbac.is_allowed_unwrap(self, action, Organization::from(*org_id))
                    .then_some(*org_id)
            })
            .collect()
    }

    pub fn projects(
        &self,
        rbac: &Rbac,
        action: bencher_rbac::project::Permission,
    ) -> Vec<ProjectId> {
        self.projects
            .iter()
            .filter_map(|org_project_id| {
                rbac.is_allowed_unwrap(self, action, Project::from(*org_project_id))
                    .then_some(org_project_id.project_id)
            })
            .collect()
    }

    #[cfg(feature = "plus")]
    pub fn rate_limit_invites(&self, context: &ApiContext) -> Result<(), HttpError> {
        context.rate_limiting.user_invite(self.user.uuid)
    }

    #[cfg(feature = "plus")]
    pub fn to_customer(&self) -> JsonCustomer {
        JsonCustomer {
            uuid: self.user.uuid,
            name: self.user.name.clone().into(),
            email: self.user.email.clone(),
        }
    }
}

impl Deref for AuthUser {
    type Target = QueryUser;

    fn deref(&self) -> &Self::Target {
        &self.user
    }
}

impl Sanitize for AuthUser {
    fn sanitize(&mut self) {
        self.user.sanitize();
    }
}

#[derive(Debug, Clone, Copy)]
pub enum AuthCredential {
    /// A `client` audience JWT from a login.
    Session,
    /// A deprecated user API token, or a user loaded internally.
    ApiToken,
    UserKey(UserKeyId),
    /// An OAuth access token, whose grant dies with the user key it was approved with, if any.
    #[cfg(feature = "plus")]
    OAuthGrant {
        user_key_id: Option<UserKeyId>,
    },
}

// https://github.com/oxidecomputer/cio/blob/master/dropshot-verify-request/src/bearer.rs
/// A bearer credential extracted from the `Authorization` header.
///
/// The two shapes are distinguished by prefix: `bencher_user_*` is a user API key,
/// anything else is parsed as a JWT.
pub enum BearerToken {
    Jwt(Jwt),
    UserKey(UserKey),
}

impl From<Jwt> for BearerToken {
    fn from(jwt: Jwt) -> Self {
        Self::Jwt(jwt)
    }
}

impl From<UserKey> for BearerToken {
    fn from(key: UserKey) -> Self {
        Self::UserKey(key)
    }
}

impl BearerToken {
    /// Parse a raw (Bearer-prefix-stripped) credential string into a `BearerToken`.
    /// Centralized so the `SharedExtractor` impl and `ApiActor`'s dispatch agree on
    /// the prefix detection rules.
    pub fn from_raw(raw: &str) -> Result<Self, HttpError> {
        // Refused before any parse, whose error would quote the token.
        #[cfg(feature = "plus")]
        if is_oauth_credential(raw) {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::OAuthTokenAuthFailed(
                bencher_otel::OAuthTokenAuthFailureReason::RestApi,
            ));
            return Err(unauthorized_error(
                "OAuth tokens are not accepted by the REST API",
            ));
        }
        if raw.starts_with(UserKey::PREFIX) {
            raw.parse::<UserKey>().map(Self::UserKey).map_err(|e| {
                // Mirror `ProjectKeyAuthFailureReason::Invalid` (emitted in
                // `actor.rs::authenticate_project_key`) so crafted
                // `bencher_user_*` probes are observable as a dedicated metric
                // and not just `bad_request_error` log lines.
                #[cfg(feature = "otel")]
                bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserKeyAuthFailed(
                    bencher_otel::UserKeyAuthFailureReason::Invalid,
                ));
                bad_request_error(format!("Malformed user API key: {e}"))
            })
        } else {
            raw.parse::<Jwt>()
                .map(Self::Jwt)
                .map_err(|e| bad_request_error(format!("Malformed JSON Web Token: {e}")))
        }
    }
}

#[async_trait]
impl SharedExtractor for BearerToken {
    async fn from_request<Context: ServerContext>(
        rqctx: &RequestContext<Context>,
    ) -> Result<Self, HttpError> {
        let headers = rqctx.request.headers();

        let Some(authorization) = headers.get(bencher_json::AUTHORIZATION) else {
            return Err(bad_request_error(format!(
                "Request is missing \"Authorization\" header. {BEARER_TOKEN_FORMAT}"
            )));
        };
        let authorization_str = match authorization.to_str() {
            Ok(authorization_str) => authorization_str,
            Err(e) => {
                return Err(bad_request_error(format!(
                    "Request has an invalid \"Authorization\" header: {e}. {BEARER_TOKEN_FORMAT}"
                )));
            },
        };
        let Some(token) = bencher_json::strip_bearer_token(authorization_str) else {
            return Err(bad_request_error(format!(
                "Request is missing \"Authorization\" Bearer. {BEARER_TOKEN_FORMAT}"
            )));
        };

        Self::from_raw(token)
    }

    fn metadata(_body_content_type: ApiEndpointBodyContentType) -> ExtractorMetadata {
        ExtractorMetadata {
            extension_mode: ExtensionMode::None,
            parameters: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct OrgProjectId {
    pub org_id: OrganizationId,
    pub project_id: ProjectId,
}

impl From<OrgProjectId> for Project {
    fn from(org_project_id: OrgProjectId) -> Self {
        Self {
            organization_id: org_project_id.org_id.to_string(),
            id: org_project_id.project_id.to_string(),
        }
    }
}

impl ToPolar for &AuthUser {
    fn to_polar(self) -> PolarValue {
        self.rbac.clone().to_polar()
    }
}
