#[cfg(feature = "plus")]
use bencher_json::PlanLevel;
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
            None => Err(TokenError::RunnerOci {
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
