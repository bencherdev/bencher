use std::str::FromStr;

use bencher_json::{organization::member::JsonOrganizationRole, Email, Jwt};
use chrono::Utc;
use derive_more::Display;
use jsonwebtoken::{decode, encode, Algorithm, Header, TokenData, Validation};
pub use jsonwebtoken::{DecodingKey, EncodingKey};
use once_cell::sync::Lazy;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ApiError;

const BENCHER_DEV: &str = "bencher.dev";

static HEADER: Lazy<Header> = Lazy::new(Header::default);
static ALGORITHM: Lazy<Algorithm> = Lazy::new(Algorithm::default);

#[derive(Debug, Display, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonWebToken(Jwt);

impl AsRef<str> for JsonWebToken {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl From<JsonWebToken> for Jwt {
    fn from(jwt: JsonWebToken) -> Self {
        jwt.0
    }
}

impl JsonWebToken {
    fn new(
        key: &EncodingKey,
        audience: Audience,
        email: Email,
        ttl: u32,
        org: Option<OrgClaims>,
    ) -> Result<Self, ApiError> {
        let claims = JsonClaims::new(audience, email, ttl, org);
        Ok(Self(Jwt::from_str(&encode(&HEADER, &claims, key)?)?))
    }

    pub fn new_auth(key: &EncodingKey, email: Email, ttl: u32) -> Result<Self, ApiError> {
        Self::new(key, Audience::Auth, email, ttl, None)
    }

    pub fn new_client(key: &EncodingKey, email: Email, ttl: u32) -> Result<Self, ApiError> {
        Self::new(key, Audience::Client, email, ttl, None)
    }

    pub fn new_api_key(key: &EncodingKey, email: Email, ttl: u32) -> Result<Self, ApiError> {
        Self::new(key, Audience::ApiKey, email, ttl, None)
    }

    pub fn new_invite(
        key: &EncodingKey,
        email: Email,
        ttl: u32,
        org_uuid: Uuid,
        role: JsonOrganizationRole,
    ) -> Result<Self, ApiError> {
        let org_claims = OrgClaims {
            uuid: org_uuid,
            role,
        };
        Self::new(key, Audience::Invite, email, ttl, Some(org_claims))
    }

    fn validate(
        token: &Jwt,
        key: &DecodingKey,
        audience: &[Audience],
    ) -> Result<TokenData<JsonClaims>, jsonwebtoken::errors::Error> {
        let mut validation = Validation::new(*ALGORITHM);
        validation.set_audience(audience);
        validation.set_issuer(&[BENCHER_DEV]);
        validation.set_required_spec_claims(&["aud", "exp", "iss"]);
        let token_data: TokenData<JsonClaims> = decode(token.as_ref(), key, &validation)?;

        // TODO deep dive on this
        // Even though the above should validate the expiration,
        // it appears to do so statically based off of compilation
        // or something, so just double check here.
        #[allow(clippy::cast_sign_loss)]
        if token_data.claims.exp < Utc::now().timestamp() as u64 {
            Err(jsonwebtoken::errors::ErrorKind::ExpiredSignature.into())
        } else {
            Ok(token_data)
        }
    }

    pub fn validate_auth(
        token: &Jwt,
        key: &DecodingKey,
    ) -> Result<TokenData<JsonClaims>, jsonwebtoken::errors::Error> {
        Self::validate(token, key, &[Audience::Auth])
    }

    pub fn validate_user(
        token: &Jwt,
        key: &DecodingKey,
    ) -> Result<TokenData<JsonClaims>, jsonwebtoken::errors::Error> {
        Self::validate(token, key, &[Audience::Client, Audience::ApiKey])
    }

    pub fn validate_api_key(
        token: &Jwt,
        key: &DecodingKey,
    ) -> Result<TokenData<JsonClaims>, jsonwebtoken::errors::Error> {
        Self::validate(token, key, &[Audience::ApiKey])
    }

    pub fn validate_invite(
        token: &Jwt,
        key: &DecodingKey,
    ) -> Result<TokenData<JsonClaims>, jsonwebtoken::errors::Error> {
        Self::validate(token, key, &[Audience::Invite])
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
    #[allow(
        clippy::arithmetic_side_effects,
        clippy::cast_sign_loss,
        clippy::integer_arithmetic
    )]
    fn new(audience: Audience, email: Email, ttl: u32, org: Option<OrgClaims>) -> Self {
        let now = Utc::now().timestamp() as u64;
        Self {
            aud: audience.into(),
            exp: now + u64::from(ttl),
            iat: now,
            iss: BENCHER_DEV.into(),
            sub: email.into(),
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
