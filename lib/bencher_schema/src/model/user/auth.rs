use std::ops::Deref;

use async_trait::async_trait;
use bencher_json::{
    DateTime, Jwt, JwtHash, Sanitize, SessionToken, SignInRefusal, UserKey, UserKeyHash,
};
#[cfg(feature = "plus")]
use bencher_json::{OAuthScopes, system::payment::JsonCustomer};
use bencher_rbac::{
    Organization, Project, Server, User as RbacUser,
    server::Permission,
    user::{OrganizationRoles, ProjectRoles},
};
use diesel::{ExpressionMethods as _, OptionalExtension as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::{
    ApiEndpointBodyContentType, ExtensionMode, ExtractorMetadata, HttpError, RequestContext,
    ServerContext, SharedExtractor,
};
use oso::{PolarValue, ToPolar};

#[cfg(feature = "plus")]
use crate::context::OAuthServer;
use crate::{
    auth_conn,
    context::{ApiContext, DbConnection, Rbac},
    error::{
        BEARER_TOKEN_FORMAT, forbidden_error, invalid_credential, issue_error, unauthorized_error,
    },
    model::{
        organization::OrganizationId,
        project::ProjectId,
        user::{
            key::{QueryUserKey, UserKeyId},
            session::{
                QuerySession, SessionId,
                cookie::{CookieCredential, SessionCookie},
            },
            token::{QueryToken, TokenId},
        },
    },
    public_conn, schema,
};

use super::QueryUser;
#[cfg(feature = "plus")]
use super::oauth::{authenticate, is_oauth_credential};

const CROSS_ORIGIN_SESSION: &str = "A session can change state only from the console's own origin";
const FRESH_SIGN_IN: &str = "This needs a browser session signed in within the last 10 minutes";

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
        Self::from_pub_token(context, bearer_token)
            .await?
            .ok_or_else(|| dead_session(&context.console_url))
    }

    /// [`Self::from_token`] for a path open to the public, where a dead session cookie leaves the
    /// request signed out, as `None`.
    pub async fn from_pub_token(
        context: &ApiContext,
        bearer_token: BearerToken,
    ) -> Result<Option<Self>, HttpError> {
        let now = context.clock.now();
        let (auth_user, renewal) = {
            // Check out a single connection for the entire auth flow:
            // credential resolution and RBAC hydration share one pool checkout.
            let conn = public_conn!(context);
            let (query_user, credential, renewal) = match bearer_token {
                BearerToken::Jwt(jwt) => {
                    let (query_user, credential, query_token) =
                        Self::user_from_jwt(context, conn, &jwt)?;
                    let renewal = query_token
                        .filter(|query_token| query_token.needs_use_recorded(now))
                        .map(|query_token| Renewal::ApiToken(query_token.id));
                    (query_user, credential, renewal)
                },
                BearerToken::UserKey(key) => {
                    let (query_user, query_key) = Self::user_from_user_key(context, conn, &key)?;
                    (query_user, AuthCredential::UserKey(query_key.id), None)
                },
                BearerToken::Session(cookie) => {
                    let Some((query_user, query_session)) =
                        Self::user_from_session(context, conn, &cookie, now)?
                    else {
                        return Ok(None);
                    };
                    let renewal = query_session
                        .needs_renewal(now)
                        .then_some(Renewal::Session(query_session.id));
                    (
                        query_user,
                        AuthCredential::Session(Some(query_session.id)),
                        renewal,
                    )
                },
            };

            #[cfg(feature = "plus")]
            context.rate_limiting.user_request(query_user.uuid)?;

            (Self::load_inner(conn, query_user, credential)?, renewal)
        };

        // Written once the read connection is back in its pool.
        if let Some(renewal) = renewal {
            Self::renew(context, renewal, now);
        }

        Ok(Some(auth_user))
    }

    /// Renew a session or record a token's use, unless the writer is busy or the write fails:
    /// a renewal never holds up or fails the request, and a later use renews instead.
    fn renew(context: &ApiContext, renewal: Renewal, now: DateTime) {
        let Ok(mut conn) = context.database.connection.try_lock() else {
            return;
        };
        let renewed = match renewal {
            Renewal::Session(session_id) => QuerySession::renew(&mut conn, session_id, now),
            Renewal::ApiToken(token_id) => QueryToken::record_use(&mut conn, token_id, now),
        };
        if let Err(e) = renewed {
            let _err = issue_error(
                "Failed to renew a credential",
                "Failed to renew a credential:",
                e,
            );
        }
    }

    /// Record a use of a deprecated user API token, at most once an hour.
    pub fn record_api_token_use(context: &ApiContext, query_token: &QueryToken, now: DateTime) {
        if query_token.needs_use_recorded(now) {
            Self::renew(context, Renewal::ApiToken(query_token.id), now);
        }
    }

    /// Authenticate by a JWT: a session from a login, by its signature, or else a deprecated
    /// user API token, by its row.
    fn user_from_jwt(
        context: &ApiContext,
        conn: &mut DbConnection,
        jwt: &Jwt,
    ) -> Result<(QueryUser, AuthCredential, Option<QueryToken>), HttpError> {
        if let Ok(claims) = context.token_key.validate_client(jwt) {
            let query_user = QueryUser::get_with_email(conn, claims.email())
                .map_err(|_err| invalid_credential())?;
            if query_user.locked {
                return Err(invalid_credential());
            }
            return Ok((query_user, AuthCredential::Session(None), None));
        }
        let (query_user, query_token) = Self::user_from_api_token(context, conn, jwt)?;
        Ok((query_user, AuthCredential::ApiToken, Some(query_token)))
    }

    /// Authenticate by a deprecated user API token: the user its row belongs to, unless the row
    /// is revoked or past its expiration, or the user is locked. Its signature and the email it
    /// names count for nothing, and a token with no row fails.
    /// Public so the OCI token endpoint (`api_auth`) checks `docker login` the same way.
    pub fn user_from_api_token(
        context: &ApiContext,
        conn: &mut DbConnection,
        jwt: &Jwt,
    ) -> Result<(QueryUser, QueryToken), HttpError> {
        let lookup = QueryToken::from_hash(conn, &JwtHash::from(jwt))
            .optional()
            .inspect_err(|err| {
                issue_error(
                    "Failed to look up an API token",
                    "Failed to look up an API token by its hash:",
                    err,
                );
            })
            .map_err(|_err| invalid_credential())?;
        let Some((query_user, query_token)) = lookup else {
            return Err(invalid_credential());
        };
        if query_token.revoked.is_some() {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserTokenRevokedUse);
            return Err(invalid_credential());
        }
        if query_token.is_expired(context.clock.now()) || query_user.locked {
            return Err(invalid_credential());
        }
        Ok((query_user, query_token))
    }

    /// Authenticate by a session cookie: one the API honors, from the console's own origin when
    /// the request changes state, of a session that has not ended, whose user is not locked.
    /// A session that is malformed, unknown, ended, or a locked user's is dead, as `None`.
    fn user_from_session(
        context: &ApiContext,
        conn: &mut DbConnection,
        cookie: &CookieCredential,
        now: DateTime,
    ) -> Result<Option<(QueryUser, QuerySession)>, HttpError> {
        let console_url = &context.console_url;
        let Some(value) = cookie.honored(console_url, &context.relay) else {
            return Err(missing_credential());
        };
        if !cookie.is_allowed_origin(console_url) {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::SessionAuthFailed(
                bencher_otel::SessionAuthFailureReason::CrossOrigin,
            ));
            return Err(forbidden_error(CROSS_ORIGIN_SESSION));
        }

        let Ok(token) = value.parse::<SessionToken>() else {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::SessionAuthFailed(
                bencher_otel::SessionAuthFailureReason::Invalid,
            ));
            return Ok(None);
        };
        let lookup = QuerySession::from_token(conn, &token)
            .optional()
            .inspect_err(|err| {
                issue_error(
                    "Failed to look up a session",
                    "Failed to look up a session by its token's hash:",
                    err,
                );
            })
            .map_err(|_err| invalid_credential())?;
        let Some((query_user, query_session)) = lookup else {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::SessionAuthFailed(
                bencher_otel::SessionAuthFailureReason::NotFound,
            ));
            return Ok(None);
        };
        if query_session.is_expired(now) {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::SessionAuthFailed(
                bencher_otel::SessionAuthFailureReason::Expired,
            ));
            return Ok(None);
        }
        if query_user.locked {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::SessionAuthFailed(
                bencher_otel::SessionAuthFailureReason::Locked,
            ));
            return Ok(None);
        }

        Ok(Some((query_user, query_session)))
    }

    /// Authenticate via a `bencher_user_*` API key.
    /// Performs a single indexed lookup that returns the key row alongside its owning
    /// user. `revoked`, `expiration`, and `user.locked` are checked in Rust so each
    /// failure mode can be reported distinctly via `UserKeyAuthFailureReason` without
    /// a second query. All adverse cases return the same opaque invalid credential
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
            .map_err(|_err| invalid_credential())?;

        let Some((query_user, query_key)) = lookup else {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserKeyAuthFailed(
                bencher_otel::UserKeyAuthFailureReason::NotFound,
            ));
            return Err(invalid_credential());
        };

        if query_key.revoked.is_some() {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserKeyAuthFailed(
                bencher_otel::UserKeyAuthFailureReason::Revoked,
            ));
            return Err(invalid_credential());
        }

        if query_key.expiration.timestamp() <= context.clock.now().timestamp() {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserKeyAuthFailed(
                bencher_otel::UserKeyAuthFailureReason::Expired,
            ));
            return Err(invalid_credential());
        }

        if query_user.locked {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserKeyAuthFailed(
                bencher_otel::UserKeyAuthFailureReason::Locked,
            ));
            return Err(invalid_credential());
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
        matches!(self.credential, AuthCredential::Session(_))
    }

    /// Refuse a browser session whose reader has not proved who they are within
    /// [`super::session::FRESH_SIGN_IN_WINDOW`], so a stolen cookie alone cannot do what this
    /// guards.
    /// A bearer credential passes, and a `client` JWT, which holds no proof time, never does.
    pub async fn check_fresh_sign_in(&self, context: &ApiContext) -> Result<(), HttpError> {
        if !self.credential.is_windowed() {
            return Ok(());
        }
        let fresh = if let Some(session_id) = self.session_id() {
            QuerySession::find(auth_conn!(context), session_id)
                .map_err(|e| {
                    issue_error(
                        "Failed to read a session",
                        "Failed to read a session's last proof:",
                        e,
                    )
                })?
                .is_some_and(|session| session.is_fresh(context.clock.now()))
        } else {
            false
        };
        if fresh {
            Ok(())
        } else {
            let mut error = forbidden_error(FRESH_SIGN_IN);
            error.error_code = Some(SignInRefusal::FreshSignIn.as_ref().to_owned());
            Err(error)
        }
    }

    /// The browser session the request authenticated with, if it presented a session cookie.
    pub fn session_id(&self) -> Option<SessionId> {
        if let AuthCredential::Session(session_id) = self.credential {
            session_id
        } else {
            None
        }
    }

    pub fn user_key_id(&self) -> Option<UserKeyId> {
        match self.credential {
            AuthCredential::UserKey(user_key_id) => Some(user_key_id),
            #[cfg(feature = "plus")]
            AuthCredential::OAuthGrant { user_key_id } => user_key_id,
            AuthCredential::Session(_) | AuthCredential::ApiToken => None,
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
    /// A browser session: a session cookie, or a `client` audience JWT from a login, which has none.
    Session(Option<SessionId>),
    /// A deprecated user API token, or a user loaded internally.
    ApiToken,
    UserKey(UserKeyId),
    /// An OAuth access token, whose grant dies with the user key it was approved with, if any.
    #[cfg(feature = "plus")]
    OAuthGrant {
        user_key_id: Option<UserKeyId>,
    },
}

impl AuthCredential {
    /// Whether the fresh sign-in window holds it: a browser session does, and a bearer
    /// credential does not.
    fn is_windowed(self) -> bool {
        match self {
            Self::Session(_) => true,
            Self::ApiToken | Self::UserKey(_) => false,
            #[cfg(feature = "plus")]
            Self::OAuthGrant { .. } => false,
        }
    }
}

/// A write a use of a credential leaves for after the request has read what it needs.
#[derive(Debug, Clone, Copy)]
enum Renewal {
    Session(SessionId),
    ApiToken(TokenId),
}

// https://github.com/oxidecomputer/cio/blob/master/dropshot-verify-request/src/bearer.rs
/// A credential extracted from the `Authorization` header, or, from a request without one,
/// its session cookie.
///
/// The two header shapes are distinguished by prefix: `bencher_user_*` is a user API key,
/// anything else is parsed as a JWT.
pub enum BearerToken {
    Jwt(Jwt),
    UserKey(UserKey),
    Session(CookieCredential),
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
        // Refused before the JWT parse so an OAuth token gets its own 401 and metric.
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
            raw.parse::<UserKey>().map(Self::UserKey).map_err(|_err| {
                // Mirror `ProjectKeyAuthFailureReason::Invalid` (emitted in
                // `actor.rs::authenticate_project_key`) so crafted
                // `bencher_user_*` probes are observable as a dedicated metric.
                #[cfg(feature = "otel")]
                bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserKeyAuthFailed(
                    bencher_otel::UserKeyAuthFailureReason::Invalid,
                ));
                invalid_credential()
            })
        } else {
            raw.parse::<Jwt>()
                .map(Self::Jwt)
                .map_err(|_err| invalid_credential())
        }
    }

    /// Whether the API honors the credential at all: a session cookie, only in the form the
    /// console's URL calls for, and only from the relay once a relay secret is set.
    pub fn is_honored(&self, context: &ApiContext) -> bool {
        if let Self::Session(cookie) = self {
            cookie
                .honored(&context.console_url, &context.relay)
                .is_some()
        } else {
            true
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
            return CookieCredential::from_request(&rqctx.request)
                .map(Self::Session)
                .ok_or_else(missing_credential);
        };
        let token = authorization
            .to_str()
            .ok()
            .and_then(bencher_json::strip_bearer_token)
            .ok_or_else(invalid_credential)?;
        Self::from_raw(token)
    }

    fn metadata(_body_content_type: ApiEndpointBodyContentType) -> ExtractorMetadata {
        ExtractorMetadata {
            extension_mode: ExtensionMode::None,
            parameters: Vec::new(),
        }
    }
}

fn missing_credential() -> HttpError {
    unauthorized_error(format!(
        "Request is missing \"Authorization\" header. {BEARER_TOKEN_FORMAT}"
    ))
}

fn dead_session(console_url: &url::Url) -> HttpError {
    let mut error = invalid_credential();
    error.headers_mut().append(
        http::header::SET_COOKIE,
        SessionCookie::new(console_url).clear(),
    );
    error
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

#[cfg(test)]
mod tests {
    use super::{AuthCredential, SessionId, UserKeyId};

    // A browser session, cookie or old login JWT, is held to the fresh sign-in window; a user
    // key, a deprecated API token, and an OAuth grant do what they did before. No endpoint takes
    // an OAuth grant but the MCP server's, whose tools do no guarded action.
    // Kills a window that holds a bearer credential, or exempts a session.
    #[test]
    fn window_holds_browser_sessions_alone() {
        for session_id in [Some(SessionId::from_raw(1)), None] {
            assert!(
                AuthCredential::Session(session_id).is_windowed(),
                "{session_id:?}"
            );
        }
        assert!(!AuthCredential::ApiToken.is_windowed());
        assert!(!AuthCredential::UserKey(UserKeyId::from_raw(1)).is_windowed());
        #[cfg(feature = "plus")]
        for user_key_id in [None, Some(UserKeyId::from_raw(1))] {
            assert!(
                !AuthCredential::OAuthGrant { user_key_id }.is_windowed(),
                "{user_key_id:?}"
            );
        }
    }
}
