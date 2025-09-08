#[cfg(feature = "plus")]
use bencher_json::PlanLevel;
use bencher_json::{
    DateTime, Email, Jwt, OrganizationUuid, organization::member::OrganizationRole,
};
use chrono::Utc;
use jsonwebtoken::errors::ErrorKind as JsonWebTokenErrorKind;
use serde::{Deserialize, Serialize};

use super::{TokenError, audience::Audience};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub aud: String,                // Audience
    pub exp: i64,                   // Expiration time (as UTC timestamp)
    pub iat: i64,                   // Issued at (as UTC timestamp)
    pub iss: String,                // Issuer
    pub sub: Email,                 // Subject (whom token refers to)
    pub org: Option<OrgClaims>,     // Organization (for invitation)
    pub state: Option<StateClaims>, // State (for OAuth)
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
