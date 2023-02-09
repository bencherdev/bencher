use std::str::FromStr;

use bencher_json::{organization::member::JsonOrganizationRole, Email, Jwt, Secret};
use chrono::Utc;
use jsonwebtoken::{decode, encode, Algorithm, Header, TokenData, Validation};
pub use jsonwebtoken::{DecodingKey, EncodingKey};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ApiError;

const BENCHER_DEV: &str = "bencher.dev";

static HEADER: Lazy<Header> = Lazy::new(Header::default);
static ALGORITHM: Lazy<Algorithm> = Lazy::new(Algorithm::default);

pub struct SecretKey {
    pub encoding: EncodingKey,
    pub decoding: DecodingKey,
}

impl From<Secret> for SecretKey {
    fn from(secret_key: Secret) -> Self {
        Self {
            encoding: EncodingKey::from_secret(secret_key.as_ref().as_bytes()),
            decoding: DecodingKey::from_secret(secret_key.as_ref().as_bytes()),
        }
    }
}

impl SecretKey {
    fn jwt(
        &self,
        audience: Audience,
        email: Email,
        ttl: u32,
        org: Option<OrgClaims>,
    ) -> Result<Jwt, ApiError> {
        let claims = JsonClaims::new(audience, email, ttl, org);
        Ok(Jwt::from_str(&encode(&HEADER, &claims, &self.encoding)?)?)
    }

    pub fn new_auth(&self, email: Email, ttl: u32) -> Result<Jwt, ApiError> {
        self.jwt(Audience::Auth, email, ttl, None)
    }

    pub fn new_client(&self, email: Email, ttl: u32) -> Result<Jwt, ApiError> {
        self.jwt(Audience::Client, email, ttl, None)
    }

    pub fn new_api_key(&self, email: Email, ttl: u32) -> Result<Jwt, ApiError> {
        self.jwt(Audience::ApiKey, email, ttl, None)
    }

    pub fn new_invite(
        &self,
        email: Email,
        ttl: u32,
        org_uuid: Uuid,
        role: JsonOrganizationRole,
    ) -> Result<Jwt, ApiError> {
        let org_claims = OrgClaims {
            uuid: org_uuid,
            role,
        };
        self.jwt(Audience::Invite, email, ttl, Some(org_claims))
    }

    fn validate(
        &self,
        token: &Jwt,
        audience: &[Audience],
    ) -> Result<TokenData<JsonClaims>, ApiError> {
        let mut validation = Validation::new(*ALGORITHM);
        validation.set_audience(audience);
        validation.set_issuer(&[BENCHER_DEV]);
        validation.set_required_spec_claims(&["aud", "exp", "iss"]);
        let token_data: TokenData<JsonClaims> =
            decode(token.as_ref(), &self.decoding, &validation)?;

        // TODO deep dive on this
        // Even though the above should validate the expiration,
        // it appears to do so statically based off of compilation
        // or something, so just double check here.
        #[allow(clippy::cast_sign_loss)]
        if token_data.claims.exp < Utc::now().timestamp() as u64 {
            Err(jsonwebtoken::errors::Error::from(
                jsonwebtoken::errors::ErrorKind::ExpiredSignature,
            )
            .into())
        } else {
            Ok(token_data)
        }
    }

    pub fn validate_auth(&self, token: &Jwt) -> Result<TokenData<JsonClaims>, ApiError> {
        self.validate(token, &[Audience::Auth])
    }

    pub fn validate_user(&self, token: &Jwt) -> Result<TokenData<JsonClaims>, ApiError> {
        self.validate(token, &[Audience::Client, Audience::ApiKey])
    }

    pub fn validate_api_key(&self, token: &Jwt) -> Result<TokenData<JsonClaims>, ApiError> {
        self.validate(token, &[Audience::ApiKey])
    }

    pub fn validate_invite(&self, token: &Jwt) -> Result<TokenData<JsonClaims>, ApiError> {
        self.validate(token, &[Audience::Invite])
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
