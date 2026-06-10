#[cfg(feature = "plus")]
use bencher_json::PlanLevel;
#[cfg(feature = "plus")]
use bencher_json::ProjectUuid;
#[cfg(feature = "plus")]
use bencher_json::RunnerUuid;
use bencher_json::{
    DateTime, Email, Jwt, OrganizationUuid, organization::member::OrganizationRole,
};
use chrono::Utc;
use jsonwebtoken::errors::ErrorKind as JsonWebTokenErrorKind;
use serde::{Deserialize, Serialize};

use super::{TokenError, audience::Audience};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub aud: String,                 // Audience
    pub exp: i64,                    // Expiration time (as UTC timestamp)
    pub iat: i64,                    // Issued at (as UTC timestamp)
    pub iss: String,                 // Issuer
    pub sub: Email,                  // Subject (whom token refers to)
    pub org: Option<OrgClaims>,      // Organization (for invitation)
    pub state: Option<StateClaims>,  // State (for OAuth)
    pub oci: Option<OciScopeClaims>, // OCI scope (for registry access)
}

/// OCI registry action
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OciAction {
    #[serde(rename = "pull")]
    Pull,
    #[serde(rename = "push")]
    Push,
}

/// OCI-specific scope claims for registry access tokens
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OciScopeClaims {
    /// Repository name (e.g., `org-slug/project-slug`)
    pub repository: Option<String>,
    /// Allowed actions
    pub actions: Vec<OciAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgClaims {
    pub uuid: OrganizationUuid,
    pub role: OrganizationRole,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateClaims {
    pub invite: Option<Jwt>,
    pub claim: Option<OrganizationUuid>,
    #[cfg(feature = "plus")]
    pub plan: Option<PlanLevel>,
}

impl Claims {
    pub fn new(
        audience: Audience,
        issuer: String,
        email: Email,
        ttl: u32,
        org: Option<OrgClaims>,
        state: Option<StateClaims>,
        oci: Option<OciScopeClaims>,
    ) -> Self {
        let now = Utc::now().timestamp();
        Self {
            aud: audience.into(),
            exp: now.checked_add(i64::from(ttl)).unwrap_or(now),
            iat: now,
            iss: issuer,
            sub: email,
            org,
            state,
            oci,
        }
    }

    pub fn audience(&self) -> &str {
        &self.aud
    }

    pub fn email(&self) -> &Email {
        &self.sub
    }

    pub fn issued_at(&self) -> DateTime {
        let date_time = DateTime::try_from(self.iat);
        debug_assert!(date_time.is_ok(), "Issued at time is invalid");
        date_time.unwrap_or_default()
    }

    pub fn expiration(&self) -> DateTime {
        let date_time = DateTime::try_from(self.exp);
        debug_assert!(date_time.is_ok(), "Expiration time is invalid");
        date_time.unwrap_or_default()
    }
}

#[derive(Debug, Clone)]
pub struct InviteClaims {
    pub aud: String,
    pub exp: i64,
    pub iat: i64,
    pub iss: String,
    pub sub: Email,
    pub org: OrgClaims,
}

impl TryFrom<Claims> for InviteClaims {
    type Error = TokenError;

    fn try_from(claims: Claims) -> Result<Self, Self::Error> {
        match claims.org {
            Some(org) => Ok(Self {
                aud: claims.aud,
                exp: claims.exp,
                iat: claims.iat,
                iss: claims.iss,
                sub: claims.sub,
                org,
            }),
            None => Err(TokenError::Invite {
                error: JsonWebTokenErrorKind::MissingRequiredClaim("org".into()).into(),
            }),
        }
    }
}

impl InviteClaims {
    pub fn email(&self) -> &Email {
        &self.sub
    }
}

#[derive(Debug, Clone)]
pub struct OAuthClaims {
    pub aud: String,
    pub exp: i64,
    pub iat: i64,
    pub iss: String,
    pub sub: Email,
    pub state: StateClaims,
}

impl TryFrom<Claims> for OAuthClaims {
    type Error = TokenError;

    fn try_from(claims: Claims) -> Result<Self, Self::Error> {
        match claims.state {
            Some(state) => Ok(Self {
                aud: claims.aud,
                exp: claims.exp,
                iat: claims.iat,
                iss: claims.iss,
                sub: claims.sub,
                state,
            }),
            None => Err(TokenError::OAuthState {
                error: JsonWebTokenErrorKind::MissingRequiredClaim("state".into()).into(),
            }),
        }
    }
}

/// Raw JWT claims for public (anonymous) OCI tokens (used for encoding/decoding).
///
/// Has no `sub` field — anonymous tokens have no identity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PublicOciTokenClaims {
    pub aud: String,
    pub exp: i64,
    pub iat: i64,
    pub iss: String,
    pub oci: Option<OciScopeClaims>,
}

/// Validated public OCI token claims.
///
/// Guarantees that the `oci` scope is present (not `Option`).
/// Has no identity — used for anonymous/unauthenticated access.
#[derive(Debug, Clone)]
pub struct PublicOciClaims {
    pub oci: OciScopeClaims,
}

impl TryFrom<PublicOciTokenClaims> for PublicOciClaims {
    type Error = TokenError;

    fn try_from(claims: PublicOciTokenClaims) -> Result<Self, Self::Error> {
        match claims.oci {
            Some(oci) => Ok(Self { oci }),
            None => Err(TokenError::OciPublic {
                error: JsonWebTokenErrorKind::MissingRequiredClaim("oci".into()).into(),
            }),
        }
    }
}

/// Validated authenticated OCI token claims.
///
/// Guarantees that the `oci` scope is present (not `Option`).
/// Contains a user email as the subject.
#[derive(Debug, Clone)]
pub struct AuthOciClaims {
    pub aud: String,
    pub exp: i64,
    pub iat: i64,
    pub iss: String,
    pub sub: Email,
    pub oci: OciScopeClaims,
}

impl TryFrom<Claims> for AuthOciClaims {
    type Error = TokenError;

    fn try_from(claims: Claims) -> Result<Self, Self::Error> {
        match claims.oci {
            Some(oci) => Ok(Self {
                aud: claims.aud,
                exp: claims.exp,
                iat: claims.iat,
                iss: claims.iss,
                sub: claims.sub,
                oci,
            }),
            None => Err(TokenError::OciAuth {
                error: JsonWebTokenErrorKind::MissingRequiredClaim("oci".into()).into(),
            }),
        }
    }
}

impl AuthOciClaims {
    pub fn email(&self) -> &Email {
        &self.sub
    }
}

/// Raw JWT claims for runner OCI tokens (used for encoding/decoding).
///
/// Uses `RunnerUuid` as the subject instead of `Email`,
/// so this cannot be confused with user OCI tokens.
#[cfg(feature = "plus")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RunnerOciTokenClaims {
    pub aud: String,
    pub exp: i64,
    pub iat: i64,
    pub iss: String,
    pub sub: RunnerUuid,
    pub oci: Option<OciScopeClaims>,
}

/// Validated runner OCI token claims.
///
/// Guarantees that the `oci` scope is present (not `Option`).
#[cfg(feature = "plus")]
#[derive(Debug, Clone)]
pub struct RunnerOciClaims {
    pub aud: String,
    pub exp: i64,
    pub iat: i64,
    pub iss: String,
    pub sub: RunnerUuid,
    pub oci: OciScopeClaims,
}

#[cfg(feature = "plus")]
impl TryFrom<RunnerOciTokenClaims> for RunnerOciClaims {
    type Error = TokenError;

    fn try_from(claims: RunnerOciTokenClaims) -> Result<Self, Self::Error> {
        match claims.oci {
            Some(oci) => Ok(Self {
                aud: claims.aud,
                exp: claims.exp,
                iat: claims.iat,
                iss: claims.iss,
                sub: claims.sub,
                oci,
            }),
            None => Err(TokenError::OciRunner {
                error: JsonWebTokenErrorKind::MissingRequiredClaim("oci".into()).into(),
            }),
        }
    }
}

#[cfg(feature = "plus")]
impl RunnerOciClaims {
    pub fn runner_uuid(&self) -> RunnerUuid {
        self.sub
    }
}

#[cfg(feature = "plus")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ProjectOciTokenClaims {
    pub aud: String,
    pub exp: i64,
    pub iat: i64,
    pub iss: String,
    pub sub: ProjectUuid,
    pub oci: Option<OciScopeClaims>,
}

#[cfg(feature = "plus")]
#[derive(Debug, Clone)]
pub struct ProjectOciClaims {
    pub aud: String,
    pub exp: i64,
    pub iat: i64,
    pub iss: String,
    pub sub: ProjectUuid,
    pub oci: OciScopeClaims,
}

#[cfg(feature = "plus")]
impl TryFrom<ProjectOciTokenClaims> for ProjectOciClaims {
    type Error = TokenError;

    fn try_from(claims: ProjectOciTokenClaims) -> Result<Self, Self::Error> {
        match claims.oci {
            Some(oci) => Ok(Self {
                aud: claims.aud,
                exp: claims.exp,
                iat: claims.iat,
                iss: claims.iss,
                sub: claims.sub,
                oci,
            }),
            None => Err(TokenError::OciProject {
                error: JsonWebTokenErrorKind::MissingRequiredClaim("oci".into()).into(),
            }),
        }
    }
}

#[cfg(feature = "plus")]
impl ProjectOciClaims {
    pub fn project_uuid(&self) -> ProjectUuid {
        self.sub
    }
}

#[cfg(test)]
mod tests {
    use std::sync::LazyLock;

    use bencher_json::{DateTime, Email, OrganizationUuid, organization::member::OrganizationRole};
    use pretty_assertions::assert_eq;

    use crate::{Audience, TokenError};

    use super::{
        AuthOciClaims, Claims, InviteClaims, OAuthClaims, OciAction, OciScopeClaims, OrgClaims,
        StateClaims,
    };

    const BENCHER_DOT_DEV_ISSUER: &str = "bencher.dev";
    const TTL: u32 = 3_600;
    /// `DateTime::TEST` as a raw UTC timestamp: 06:23 UTC, 11 July 2024.
    const TEST_TIMESTAMP: i64 = 1_720_678_980;

    static EMAIL: LazyLock<Email> = LazyLock::new(|| "info@bencher.dev".parse().unwrap());

    /// Build `Claims` with a fixed, deterministic issued-at time (`DateTime::TEST`).
    ///
    /// `Claims::new` reads the wall clock internally (no clock injection),
    /// so deterministic tests construct the struct directly instead.
    fn fixed_claims(
        org: Option<OrgClaims>,
        state: Option<StateClaims>,
        oci: Option<OciScopeClaims>,
    ) -> Claims {
        Claims {
            aud: Audience::Auth.into(),
            exp: TEST_TIMESTAMP + i64::from(TTL),
            iat: TEST_TIMESTAMP,
            iss: BENCHER_DOT_DEV_ISSUER.to_owned(),
            sub: EMAIL.clone(),
            org,
            state,
            oci,
        }
    }

    fn org_claims() -> OrgClaims {
        OrgClaims {
            uuid: OrganizationUuid::new(),
            role: OrganizationRole::Leader,
        }
    }

    fn state_claims() -> StateClaims {
        StateClaims {
            invite: None,
            claim: Some(OrganizationUuid::new()),
            #[cfg(feature = "plus")]
            plan: None,
        }
    }

    // `Claims::new` uses the wall clock internally, so only relative
    // (`exp` - `iat`) and non-time fields are asserted here.

    #[test]
    fn claims_new_exp_is_iat_plus_ttl() {
        let claims = Claims::new(
            Audience::Auth,
            BENCHER_DOT_DEV_ISSUER.to_owned(),
            EMAIL.clone(),
            TTL,
            None,
            None,
            None,
        );

        assert_eq!(claims.exp - claims.iat, i64::from(TTL));
        assert_eq!(claims.aud, Audience::Auth.as_str());
        assert_eq!(claims.iss, BENCHER_DOT_DEV_ISSUER);
        assert_eq!(claims.sub, *EMAIL);
        assert!(claims.org.is_none());
        assert!(claims.state.is_none());
        assert!(claims.oci.is_none());
    }

    #[test]
    fn claims_new_zero_ttl_expires_at_issued_at() {
        let claims = Claims::new(
            Audience::Client,
            BENCHER_DOT_DEV_ISSUER.to_owned(),
            EMAIL.clone(),
            0,
            None,
            None,
            None,
        );

        assert_eq!(claims.exp, claims.iat);
    }

    #[test]
    fn claims_accessors() {
        let claims = fixed_claims(None, None, None);

        assert_eq!(claims.audience(), Audience::Auth.as_str());
        assert_eq!(claims.email(), &*EMAIL);
    }

    // --- issued_at()/expiration() timestamp conversion ---

    #[test]
    fn claims_issued_at_round_trips_fixed_timestamp() {
        let claims = fixed_claims(None, None, None);

        assert_eq!(claims.issued_at(), DateTime::TEST);
        assert_eq!(claims.issued_at().timestamp(), TEST_TIMESTAMP);
    }

    #[test]
    fn claims_expiration_round_trips_fixed_timestamp() {
        let claims = fixed_claims(None, None, None);

        assert_eq!(
            claims.expiration(),
            DateTime::TEST + chrono::Duration::seconds(i64::from(TTL))
        );
        assert_eq!(
            claims.expiration().timestamp(),
            TEST_TIMESTAMP + i64::from(TTL)
        );
    }

    #[test]
    fn claims_issued_at_unix_epoch() {
        let mut claims = fixed_claims(None, None, None);
        claims.iat = 0;

        assert_eq!(claims.issued_at().timestamp(), 0);
    }

    #[test]
    fn claims_issued_at_pre_epoch_timestamp() {
        let mut claims = fixed_claims(None, None, None);
        claims.iat = -1;

        assert_eq!(claims.issued_at().timestamp(), -1);
    }

    // In debug builds an out-of-range timestamp trips the `debug_assert!`
    // in `issued_at()`/`expiration()`. In release builds these methods
    // would instead silently fall back to `DateTime::default()`.

    #[test]
    #[should_panic(expected = "Issued at time is invalid")]
    fn claims_issued_at_out_of_range_panics_in_debug() {
        let mut claims = fixed_claims(None, None, None);
        claims.iat = i64::MAX;

        let _date_time = claims.issued_at();
    }

    #[test]
    #[should_panic(expected = "Expiration time is invalid")]
    fn claims_expiration_out_of_range_panics_in_debug() {
        let mut claims = fixed_claims(None, None, None);
        claims.exp = i64::MAX;

        let _date_time = claims.expiration();
    }

    // --- TryFrom<Claims> for InviteClaims ---

    #[test]
    fn invite_claims_try_from_with_org() {
        let org = org_claims();
        let org_uuid = org.uuid;
        let claims = fixed_claims(Some(org), None, None);

        let invite = InviteClaims::try_from(claims).unwrap();

        assert_eq!(invite.aud, Audience::Auth.as_str());
        assert_eq!(invite.exp, TEST_TIMESTAMP + i64::from(TTL));
        assert_eq!(invite.iat, TEST_TIMESTAMP);
        assert_eq!(invite.iss, BENCHER_DOT_DEV_ISSUER);
        assert_eq!(invite.sub, *EMAIL);
        assert_eq!(invite.email(), &*EMAIL);
        assert_eq!(invite.org.uuid, org_uuid);
        assert_eq!(invite.org.role, OrganizationRole::Leader);
    }

    #[test]
    fn invite_claims_try_from_without_org() {
        let claims = fixed_claims(None, None, None);

        let error = InviteClaims::try_from(claims).unwrap_err();

        assert!(
            matches!(error, TokenError::Invite { .. }),
            "expected TokenError::Invite, got: {error:?}"
        );
    }

    // --- TryFrom<Claims> for OAuthClaims ---

    #[test]
    fn oauth_claims_try_from_with_state() {
        let state = state_claims();
        let claim_uuid = state.claim;
        let claims = fixed_claims(None, Some(state), None);

        let oauth = OAuthClaims::try_from(claims).unwrap();

        assert_eq!(oauth.aud, Audience::Auth.as_str());
        assert_eq!(oauth.exp, TEST_TIMESTAMP + i64::from(TTL));
        assert_eq!(oauth.iat, TEST_TIMESTAMP);
        assert_eq!(oauth.iss, BENCHER_DOT_DEV_ISSUER);
        assert_eq!(oauth.sub, *EMAIL);
        assert!(oauth.state.invite.is_none());
        assert_eq!(oauth.state.claim, claim_uuid);
    }

    #[test]
    fn oauth_claims_try_from_without_state() {
        let claims = fixed_claims(None, None, None);

        let error = OAuthClaims::try_from(claims).unwrap_err();

        assert!(
            matches!(error, TokenError::OAuthState { .. }),
            "expected TokenError::OAuthState, got: {error:?}"
        );
    }

    // --- TryFrom<Claims> for AuthOciClaims ---

    #[test]
    fn auth_oci_claims_try_from_with_oci() {
        let oci = OciScopeClaims {
            repository: Some("test-org/test-project".to_owned()),
            actions: vec![OciAction::Pull, OciAction::Push],
        };
        let claims = fixed_claims(None, None, Some(oci));

        let auth_oci = AuthOciClaims::try_from(claims).unwrap();

        assert_eq!(auth_oci.sub, *EMAIL);
        assert_eq!(auth_oci.email(), &*EMAIL);
        assert_eq!(
            auth_oci.oci.repository,
            Some("test-org/test-project".to_owned())
        );
        assert_eq!(auth_oci.oci.actions, vec![OciAction::Pull, OciAction::Push]);
    }

    #[test]
    fn auth_oci_claims_try_from_without_oci() {
        let claims = fixed_claims(None, None, None);

        let error = AuthOciClaims::try_from(claims).unwrap_err();

        assert!(
            matches!(error, TokenError::OciAuth { .. }),
            "expected TokenError::OciAuth, got: {error:?}"
        );
    }

    // --- OciScopeClaims serialization shape ---

    #[test]
    fn oci_scope_claims_serialize_with_repository() {
        let scope = OciScopeClaims {
            repository: Some("test-org/test-project".to_owned()),
            actions: vec![OciAction::Pull, OciAction::Push],
        };

        let value = serde_json::to_value(&scope).unwrap();

        assert_eq!(
            value,
            serde_json::json!({
                "repository": "test-org/test-project",
                "actions": ["pull", "push"],
            })
        );
    }

    #[test]
    fn oci_scope_claims_serialize_without_repository() {
        let scope = OciScopeClaims {
            repository: None,
            actions: vec![],
        };

        let value = serde_json::to_value(&scope).unwrap();

        assert_eq!(
            value,
            serde_json::json!({
                "repository": null,
                "actions": [],
            })
        );
    }

    #[test]
    fn oci_scope_claims_deserialize_round_trip() {
        let json = serde_json::json!({
            "repository": "test-org/test-project",
            "actions": ["push", "pull"],
        });

        let scope: OciScopeClaims = serde_json::from_value(json).unwrap();

        assert_eq!(scope.repository, Some("test-org/test-project".to_owned()));
        assert_eq!(scope.actions, vec![OciAction::Push, OciAction::Pull]);
    }
}
