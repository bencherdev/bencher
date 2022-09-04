use chrono::Utc;
use jsonwebtoken::{
    decode,
    encode,
    Algorithm,
    DecodingKey,
    EncodingKey,
    Header,
    TokenData,
    Validation,
};
use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{
    Deserialize,
    Serialize,
};

const BENCHER_DEV: &str = "bencher.dev";
// 15 minutes * 60 seconds / minute
const AUTH_TOKEN_TTL: usize = 15 * 60;
// 21 days * 24 hours / day * 60 minutes / hour * 60 seconds / minute
const CLIENT_TOKEN_TTL: usize = 21 * 24 * 60 * 60;

lazy_static::lazy_static! {
    static ref HEADER: Header = Header::default();
    static ref ALGORITHM: Algorithm = Algorithm::default();
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonToken {
    pub token: JsonWebToken,
}

#[derive(Debug, Display, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonWebToken(pub String);

impl From<String> for JsonWebToken {
    fn from(token: String) -> Self {
        Self(token)
    }
}

impl JsonWebToken {
    pub fn new(
        key: &str,
        audience: Audience,
        email: String,
        ttl: usize,
    ) -> Result<Self, jsonwebtoken::errors::Error> {
        let claims = JsonClaims::new(audience, email, ttl);
        encode(&*HEADER, &claims, &EncodingKey::from_secret(key.as_bytes())).map(Into::into)
    }

    pub fn new_auth(key: &str, email: String) -> Result<Self, jsonwebtoken::errors::Error> {
        Self::new(key, Audience::Auth, email, AUTH_TOKEN_TTL)
    }

    pub fn new_client(key: &str, email: String) -> Result<Self, jsonwebtoken::errors::Error> {
        Self::new(key, Audience::Client, email, CLIENT_TOKEN_TTL)
    }

    pub fn new_api_key(
        key: &str,
        email: String,
        ttl: usize,
    ) -> Result<Self, jsonwebtoken::errors::Error> {
        Self::new(key, Audience::ApiKey, email, ttl)
    }

    pub fn validate(
        &self,
        key: &str,
        audience: &[Audience],
    ) -> Result<TokenData<JsonClaims>, jsonwebtoken::errors::Error> {
        let mut validation = Validation::new(*ALGORITHM);
        validation.set_audience(audience);
        validation.set_issuer(&[BENCHER_DEV]);
        validation.set_required_spec_claims(&["aud", "exp", "iss"]);
        decode(
            &self.0,
            &DecodingKey::from_secret(key.as_bytes()),
            &validation,
        )
    }

    pub fn validate_auth(
        &self,
        key: &str,
    ) -> Result<TokenData<JsonClaims>, jsonwebtoken::errors::Error> {
        self.validate(key, &[Audience::Auth])
    }

    pub fn validate_user(
        &self,
        key: &str,
    ) -> Result<TokenData<JsonClaims>, jsonwebtoken::errors::Error> {
        self.validate(key, &[Audience::Client, Audience::ApiKey])
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonClaims {
    pub aud: String, // Audience
    pub exp: usize,  // Expiration time (as UTC timestamp)
    pub iat: usize,  // Issued at (as UTC timestamp)
    pub iss: String, // Issuer
    pub sub: String, // Subject (whom token refers to)
}

impl JsonClaims {
    fn new(audience: Audience, email: String, ttl: usize) -> Self {
        let now = Utc::now().timestamp() as usize;
        Self {
            aud: audience.into(),
            exp: now + ttl,
            iat: now,
            iss: BENCHER_DEV.into(),
            sub: email,
        }
    }

    pub fn email(&self) -> &str {
        &self.sub
    }
}

#[derive(Debug, Copy, Clone)]
pub enum Audience {
    Auth,
    Client,
    ApiKey,
}

const AUDIENCE_AUTH: &str = "auth";
const AUDIENCE_CLIENT: &str = "client";
const AUDIENCE_API_KEY: &str = "api_key";

impl ToString for Audience {
    fn to_string(&self) -> String {
        match self {
            Self::Auth => AUDIENCE_AUTH.into(),
            Self::Client => AUDIENCE_CLIENT.into(),
            Self::ApiKey => AUDIENCE_API_KEY.into(),
        }
    }
}

impl Into<String> for Audience {
    fn into(self) -> String {
        self.to_string()
    }
}
