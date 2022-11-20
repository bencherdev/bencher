use chrono::Utc;
use derive_more::Display;
use jsonwebtoken::{decode, encode, Algorithm, Header, TokenData, Validation};
pub use jsonwebtoken::{DecodingKey, EncodingKey};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::organization::member::JsonOrganizationRole;

const BENCHER_DEV: &str = "bencher.dev";

lazy_static::lazy_static! {
    static ref HEADER: Header = Header::default();
    static ref ALGORITHM: Algorithm = Algorithm::default();
}

#[derive(Debug, Display, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonWebToken(pub String);

impl From<String> for JsonWebToken {
    fn from(token: String) -> Self {
        Self(token)
    }
}

impl AsRef<str> for JsonWebToken {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl JsonWebToken {
    fn new(
        key: &EncodingKey,
        audience: Audience,
        email: String,
        ttl: u32,
        org: Option<OrgClaims>,
    ) -> Result<Self, jsonwebtoken::errors::Error> {
        let claims = JsonClaims::new(audience, email, ttl, org);
        encode(&HEADER, &claims, key).map(Into::into)
    }

    pub fn new_auth(
        key: &EncodingKey,
        email: String,
        ttl: u32,
    ) -> Result<Self, jsonwebtoken::errors::Error> {
        Self::new(key, Audience::Auth, email, ttl, None)
    }

    pub fn new_client(
        key: &EncodingKey,
        email: String,
        ttl: u32,
    ) -> Result<Self, jsonwebtoken::errors::Error> {
        Self::new(key, Audience::Client, email, ttl, None)
    }

    pub fn new_api_key(
        key: &EncodingKey,
        email: String,
        ttl: u32,
    ) -> Result<Self, jsonwebtoken::errors::Error> {
        Self::new(key, Audience::ApiKey, email, ttl, None)
    }

    pub fn new_invite(
        key: &EncodingKey,
        email: String,
        ttl: u32,
        org_uuid: Uuid,
        role: JsonOrganizationRole,
    ) -> Result<Self, jsonwebtoken::errors::Error> {
        let org_claims = OrgClaims {
            uuid: org_uuid,
            role,
        };
        Self::new(key, Audience::Invite, email, ttl, Some(org_claims))
    }

    fn validate(
        &self,
        key: &DecodingKey,
        audience: &[Audience],
    ) -> Result<TokenData<JsonClaims>, jsonwebtoken::errors::Error> {
        let mut validation = Validation::new(*ALGORITHM);
        validation.set_audience(audience);
        validation.set_issuer(&[BENCHER_DEV]);
        validation.set_required_spec_claims(&["aud", "exp", "iss"]);
        decode(&self.0, key, &validation)
    }

    pub fn validate_auth(
        &self,
        key: &DecodingKey,
    ) -> Result<TokenData<JsonClaims>, jsonwebtoken::errors::Error> {
        self.validate(key, &[Audience::Auth])
    }

    pub fn validate_user(
        &self,
        key: &DecodingKey,
    ) -> Result<TokenData<JsonClaims>, jsonwebtoken::errors::Error> {
        self.validate(key, &[Audience::Client, Audience::ApiKey])
    }

    pub fn validate_api_key(
        &self,
        key: &DecodingKey,
    ) -> Result<TokenData<JsonClaims>, jsonwebtoken::errors::Error> {
        self.validate(key, &[Audience::ApiKey])
    }

    pub fn validate_invite(
        &self,
        key: &DecodingKey,
    ) -> Result<TokenData<JsonClaims>, jsonwebtoken::errors::Error> {
        self.validate(key, &[Audience::Invite])
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonClaims {
    pub aud: String,            // Audience
    pub exp: u64,               // Expiration time (as UTC timestamp)
    pub iat: u64,               // Issued at (as UTC timestamp)
    pub iss: String,            // Issuer
    pub sub: String,            // Subject (whom token refers to)
    pub org: Option<OrgClaims>, // Organization (for invitation)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct OrgClaims {
    pub uuid: Uuid,
    pub role: JsonOrganizationRole,
}

impl JsonClaims {
    fn new(audience: Audience, email: String, ttl: u32, org: Option<OrgClaims>) -> Self {
        let now = Utc::now().timestamp() as u64;
        Self {
            aud: audience.into(),
            exp: now + ttl as u64,
            iat: now,
            iss: BENCHER_DEV.into(),
            sub: email,
            org,
        }
    }

    pub fn email(&self) -> &str {
        &self.sub
    }

    pub fn org(&self) -> Option<&OrgClaims> {
        self.org.as_ref()
    }
}

#[derive(Debug, Copy, Clone)]
pub enum Audience {
    Auth,
    Client,
    ApiKey,
    Invite,
}

const AUDIENCE_AUTH: &str = "auth";
const AUDIENCE_CLIENT: &str = "client";
const AUDIENCE_API_KEY: &str = "api_key";
const AUDIENCE_INVITE: &str = "invite";

impl ToString for Audience {
    fn to_string(&self) -> String {
        match self {
            Self::Auth => AUDIENCE_AUTH.into(),
            Self::Client => AUDIENCE_CLIENT.into(),
            Self::ApiKey => AUDIENCE_API_KEY.into(),
            Self::Invite => AUDIENCE_INVITE.into(),
        }
    }
}

impl From<Audience> for String {
    fn from(audience: Audience) -> Self {
        audience.to_string()
    }
}
