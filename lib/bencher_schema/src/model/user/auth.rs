use std::ops::Deref;

use async_trait::async_trait;
#[cfg(feature = "plus")]
use bencher_json::system::payment::JsonCustomer;
use bencher_json::{DateTime, Jwt, Sanitize};
use bencher_rbac::{
    Organization, Project, Server, User as RbacUser,
    server::Permission,
    user::{OrganizationRoles, ProjectRoles},
};
use bencher_token::{Audience, Claims, KeyMatch, TokenError, TokenKey};
use diesel::{ExpressionMethods as _, OptionalExtension as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::{
    ApiEndpointBodyContentType, ExtensionMode, ExtractorMetadata, HttpError, RequestContext,
    ServerContext, SharedExtractor,
};
use oso::{PolarValue, ToPolar};

use crate::{
    context::{ApiContext, DbConnection, Rbac},
    error::{BEARER_TOKEN_FORMAT, bad_request_error, unauthorized_error},
    model::{organization::OrganizationId, project::ProjectId, user::token::QueryToken},
    public_conn, schema,
};

use super::QueryUser;

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user: QueryUser,
    pub organizations: Vec<OrganizationId>,
    pub projects: Vec<OrgProjectId>,
    pub rbac: RbacUser,
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
        let (claims, key_match) = validate_bearer_with_rotation(&context.token_key, &bearer_token)
            .map_err(|e| bad_request_error(format!("Failed to validate JSON Web Token: {e}")))?;
        let email = claims.email();

        let conn = public_conn!(context);
        let query_user = QueryUser::get_with_email(conn, email)?;
        query_user.check_is_locked()?;

        // API keys created via `POST /tokens` are persisted in the `token` table and
        // can be revoked. Reject any JWT whose row has `revoked` set. A JWT with no
        // row (e.g. one issued before revocation tracking existed, or any
        // externally-signed test fixture) is accepted — we only block tokens we know
        // have been revoked. Short-lived client (browser session) JWTs are never
        // persisted and skip this lookup.
        if claims.audience() == Audience::ApiKey.as_str() {
            let row = QueryToken::get_by_user_jwt(conn, query_user.id, &bearer_token)
                .optional()
                .map_err(|e| unauthorized_error(format!("Failed to look up API token: {e}")))?;
            if let Some(row) = row.as_ref()
                && row.revoked.is_some()
            {
                #[cfg(feature = "otel")]
                bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserTokenRevokedUse);
                return Err(unauthorized_error(
                    "This API token has been revoked and is no longer valid",
                ));
            }
            // When the token validated against a previous (rotated-out) signing key,
            // also require the persisted DB `creation` to fall inside that key's
            // active window. This blocks a holder of a leaked previous key from
            // forging a JWT with a backdated `iat`: the JWT signature may be valid
            // and the (forged) `iat` may sit inside the window, but the real row's
            // immutable `creation` will betray the forgery.
            if let KeyMatch::Previous { creation, retired } = key_match
                && let Some(row) = row
                && !creation_in_window(row.creation, creation, retired)
            {
                #[cfg(feature = "otel")]
                bencher_otel::ApiMeter::increment(
                    bencher_otel::ApiCounter::UserTokenPreviousKeyMismatch,
                );
                return Err(unauthorized_error(
                    "This API token was not issued under the matched signing key",
                ));
            }
        }

        #[cfg(feature = "plus")]
        context.rate_limiting.user_request(query_user.uuid)?;

        Self::load(conn, query_user)
    }

    pub fn reload(&self, conn: &mut DbConnection) -> Result<Self, HttpError> {
        Self::load(conn, self.user.clone())
    }

    pub fn load(conn: &mut DbConnection, query_user: QueryUser) -> Result<Self, HttpError> {
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
                crate::error::issue_error(
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
                    let _err = crate::error::issue_error(
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
                crate::error::issue_error(
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
                    let _err = crate::error::issue_error(
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
        rbac.is_allowed_unwrap(self, Permission::Administer, Server {})
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

// https://github.com/oxidecomputer/cio/blob/master/dropshot-verify-request/src/bearer.rs
pub struct BearerToken(Jwt);

impl From<Jwt> for BearerToken {
    fn from(jwt: Jwt) -> Self {
        Self(jwt)
    }
}

impl Deref for BearerToken {
    type Target = Jwt;

    fn deref(&self) -> &Self::Target {
        &self.0
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

        token
            .parse::<Jwt>()
            .map(Into::into)
            .map_err(|e| bad_request_error(format!("Malformed JSON Web Token: {e}")))
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

/// Validate a bearer JWT against `token_key`, applying the no-DB-no-rotation
/// rule documented on `TokenKey::decode_with_rotation`.
///
/// 1. The current signing key validates either `Audience::Client` (a browser
///    session JWT, no DB row) or `Audience::ApiKey` (a DB-anchored API token).
/// 2. If step 1 fails with `InvalidSignature`, retry with
///    [`TokenKey::validate_api_key_with_match`] — which consults retired keys
///    but accepts only `Audience::ApiKey`. A `Client` JWT signed by a retired
///    key cannot resurrect itself here because the rotation validator rejects
///    its audience.
///
/// On success the caller learns which key validated the token via [`KeyMatch`]
/// so the API-token path can apply the DB-row `creation` anchor when a
/// previous key matched.
fn validate_bearer_with_rotation(
    token_key: &TokenKey,
    jwt: &Jwt,
) -> Result<(Claims, KeyMatch), TokenError> {
    match token_key.validate_client(jwt) {
        Ok(claims) => Ok((claims, KeyMatch::Current)),
        Err(e) if e.is_invalid_signature() => token_key.validate_api_key_with_match(jwt),
        Err(e) => Err(e),
    }
}

/// True iff `row_creation` falls inside the inclusive `[creation, retired]`
/// window of a previous (rotated-out) signing key. Used by [`AuthUser::from_token`]
/// to reject forged JWTs that claim a `iat` inside the window but whose
/// persisted DB row betrays the forgery.
fn creation_in_window(row_creation: DateTime, creation: DateTime, retired: DateTime) -> bool {
    let row = row_creation.timestamp();
    row >= creation.timestamp() && row <= retired.timestamp()
}

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;
    use std::sync::LazyLock;

    use bencher_json::{DateTime, Email, Jwt, Secret, system::config::JsonPreviousSecretKey};
    use bencher_token::{Audience, Claims, KeyMatch, TokenKey};
    use chrono::Duration;

    use super::{creation_in_window, validate_bearer_with_rotation};

    const ISSUER: &str = "bencher.dev";
    const TTL: u32 = u32::MAX;

    static EMAIL: LazyLock<Email> = LazyLock::new(|| "info@bencher.dev".parse().unwrap());

    fn window_start() -> DateTime {
        DateTime::TEST - Duration::seconds(3600)
    }
    fn window_end() -> DateTime {
        DateTime::TEST + Duration::seconds(3600)
    }

    #[test]
    fn row_creation_inside_window_accepted() {
        let row = DateTime::TEST;
        assert!(creation_in_window(row, window_start(), window_end()));
    }

    #[test]
    fn row_creation_at_window_start_accepted() {
        let row = window_start();
        assert!(creation_in_window(row, window_start(), window_end()));
    }

    #[test]
    fn row_creation_at_window_end_accepted() {
        let row = window_end();
        assert!(creation_in_window(row, window_start(), window_end()));
    }

    #[test]
    fn row_creation_before_window_rejected() {
        let row = window_start() - Duration::seconds(1);
        assert!(!creation_in_window(row, window_start(), window_end()));
    }

    #[test]
    fn row_creation_after_window_rejected() {
        // Forged-JWT-with-leaked-previous-key case: the JWT's `iat` may sit
        // inside the window (attacker forged it), but the real DB row was
        // created after the key was retired — the forgery must be rejected.
        let row = window_end() + Duration::seconds(1);
        assert!(!creation_in_window(row, window_start(), window_end()));
    }

    // --- Orchestration tests for `validate_bearer_with_rotation` ---
    //
    // These cover the two-step retry used by `AuthUser::from_token`: try the
    // current key for either `Client` or `ApiKey` audience first, then fall
    // back to the rotation path with `ApiKey` audience only.

    fn old_secret() -> Secret {
        "auth-rotation-old-secret".parse().unwrap()
    }
    fn new_secret() -> Secret {
        "auth-rotation-new-secret".parse().unwrap()
    }

    fn previous_entry(secret: Secret) -> JsonPreviousSecretKey {
        JsonPreviousSecretKey {
            secret_key: secret,
            creation: window_start(),
            retired: window_end(),
        }
    }

    fn rotated_key() -> TokenKey {
        TokenKey::new_with_previous(
            ISSUER.to_owned(),
            &new_secret(),
            &[previous_entry(old_secret())],
        )
    }

    fn mint_with_iat(secret: &Secret, audience: Audience, iat: i64, exp: i64) -> Jwt {
        // Hand-rolled JWT minting so we can dictate `iat`; the production
        // `TokenKey::new_*` helpers use `Utc::now()`.
        let claims = Claims {
            aud: audience.to_string(),
            exp,
            iat,
            iss: ISSUER.to_owned(),
            sub: EMAIL.clone(),
            org: None,
            state: None,
            oci: None,
        };
        let encoding = jsonwebtoken::EncodingKey::from_secret(secret.as_ref().as_bytes());
        let header = jsonwebtoken::Header::default();
        Jwt::from_str(&jsonwebtoken::encode(&header, &claims, &encoding).unwrap()).unwrap()
    }

    fn far_future_exp() -> i64 {
        chrono::Utc::now().timestamp() + 86_400
    }

    #[test]
    fn bearer_current_key_client_audience_returns_current() {
        let key = rotated_key();
        let jwt = key.new_client(EMAIL.clone(), TTL).unwrap();
        let (claims, key_match) = validate_bearer_with_rotation(&key, &jwt).unwrap();
        assert_eq!(claims.audience(), Audience::Client.as_str());
        assert!(matches!(key_match, KeyMatch::Current));
    }

    #[test]
    fn bearer_current_key_api_key_audience_returns_current() {
        let key = rotated_key();
        let jwt = key.new_api_key(EMAIL.clone(), TTL).unwrap();
        let (claims, key_match) = validate_bearer_with_rotation(&key, &jwt).unwrap();
        assert_eq!(claims.audience(), Audience::ApiKey.as_str());
        assert!(matches!(key_match, KeyMatch::Current));
    }

    #[test]
    fn bearer_previous_key_api_key_audience_returns_previous() {
        // An API token signed with the retired key, `iat` inside the window:
        // step 1 (`validate_client`, current key only) fails with invalid
        // signature → step 2 (`validate_api_key_with_match`) succeeds and
        // returns `KeyMatch::Previous`.
        let iat = DateTime::TEST.timestamp();
        let jwt = mint_with_iat(&old_secret(), Audience::ApiKey, iat, far_future_exp());
        let (claims, key_match) = validate_bearer_with_rotation(&rotated_key(), &jwt).unwrap();
        assert_eq!(claims.audience(), Audience::ApiKey.as_str());
        match key_match {
            KeyMatch::Previous { creation, retired } => {
                assert_eq!(creation.timestamp(), window_start().timestamp());
                assert_eq!(retired.timestamp(), window_end().timestamp());
            },
            KeyMatch::Current => panic!("expected KeyMatch::Previous, got Current"),
        }
    }

    #[test]
    fn bearer_previous_key_client_audience_rejected() {
        // The constraint at work: a `Client`-audience JWT signed with the
        // retired key MUST NOT validate. Step 1 fails (invalid signature),
        // step 2 fails (the rotation path accepts only `ApiKey` audience).
        let iat = DateTime::TEST.timestamp();
        let jwt = mint_with_iat(&old_secret(), Audience::Client, iat, far_future_exp());
        validate_bearer_with_rotation(&rotated_key(), &jwt).unwrap_err();
    }

    #[test]
    fn bearer_unknown_key_rejected() {
        let iat = DateTime::TEST.timestamp();
        let unknown: Secret = "auth-rotation-unknown-secret".parse().unwrap();
        let jwt = mint_with_iat(&unknown, Audience::ApiKey, iat, far_future_exp());
        validate_bearer_with_rotation(&rotated_key(), &jwt).unwrap_err();
    }

    #[test]
    fn bearer_previous_key_api_key_after_window_rejected() {
        // Post-retirement forgery: signature is valid against the retired
        // key, but `iat` sits outside `[creation, retired]`. The rotation
        // path's iat window check rejects.
        let iat = window_end().timestamp() + 1;
        let jwt = mint_with_iat(&old_secret(), Audience::ApiKey, iat, far_future_exp());
        validate_bearer_with_rotation(&rotated_key(), &jwt).unwrap_err();
    }
}
