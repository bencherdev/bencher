use bencher_json::{organization::member::OrganizationRole, DateTime, Email, OrganizationUuid};
use serde::{Deserialize, Serialize};

use super::{audience::Audience, now, JwtError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub aud: String,            // Audience
    pub exp: u64,               // Expiration time (as UTC timestamp)
    pub iat: u64,               // Issued at (as UTC timestamp)
    pub iss: String,            // Issuer
    pub sub: String,            // Subject (whom token refers to)
    pub org: Option<OrgClaims>, // Organization (for invitation)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgClaims {
    pub uuid: OrganizationUuid,
    pub role: OrganizationRole,
}

impl Claims {
    pub fn new(
        audience: Audience,
        issuer: String,
        email: Email,
        ttl: u32,
        org: Option<OrgClaims>,
    ) -> Self {
        let now = now();
        Self {
            aud: audience.into(),
            exp: now.checked_add(u64::from(ttl)).unwrap_or(now),
            iat: now,
            iss: issuer,
            sub: email.into(),
            org,
        }
    }

    pub fn email(&self) -> &str {
        &self.sub
    }

    pub fn issued_at(&self) -> DateTime {
        let timestamp = i64::try_from(self.iat);
        debug_assert!(timestamp.is_ok(), "Issued at time is invalid");
        let date_time = DateTime::try_from(timestamp.unwrap_or_default());
        debug_assert!(date_time.is_ok(), "Issued at time is invalid");
        date_time.unwrap_or_default()
    }

    pub fn expiration(&self) -> DateTime {
        let timestamp = i64::try_from(self.exp);
        debug_assert!(timestamp.is_ok(), "Expiration time is invalid");
        let date_time = DateTime::try_from(timestamp.unwrap_or_default());
        debug_assert!(date_time.is_ok(), "Expiration time is invalid");
        date_time.unwrap_or_default()
    }
}

pub struct InviteClaims {
    pub aud: String,
    pub exp: u64,
    pub iat: u64,
    pub iss: String,
    pub sub: String,
    pub org: OrgClaims,
}

impl TryFrom<Claims> for InviteClaims {
    type Error = JwtError;

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
            None => Err(JwtError::Invite {
                error: jsonwebtoken::errors::ErrorKind::MissingRequiredClaim("org".into()).into(),
            }),
        }
    }
}

impl InviteClaims {
    pub fn email(&self) -> &str {
        &self.sub
    }
}
