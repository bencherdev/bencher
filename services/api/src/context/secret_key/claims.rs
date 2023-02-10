use bencher_json::{organization::member::JsonOrganizationRole, Email};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ApiError;

use super::{audience::Audience, now};

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
    pub uuid: Uuid,
    pub role: JsonOrganizationRole,
}

impl Claims {
    pub fn new(
        audience: Audience,
        issuer: String,
        email: Email,
        ttl: u32,
        org: Option<OrgClaims>,
    ) -> Result<Self, ApiError> {
        let now = now()?;
        Ok(Self {
            aud: audience.into(),
            exp: now.checked_add(u64::from(ttl)).unwrap_or(now),
            iat: now,
            iss: issuer,
            sub: email.into(),
            org,
        })
    }

    pub fn email(&self) -> &str {
        &self.sub
    }

    pub fn org(&self) -> Option<&OrgClaims> {
        self.org.as_ref()
    }
}
