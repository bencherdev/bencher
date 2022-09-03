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
const WEB_TOKEN_TTL: usize = 21 * 24 * 60 * 60;

lazy_static::lazy_static! {
    static ref HEADER: Header = Header::default();
    static ref ALGORITHM: Algorithm = Algorithm::default();
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonToken(pub String);

impl JsonToken {
    pub fn new(
        key: &str,
        audience: Audience,
        email: String,
        ttl: usize,
    ) -> Result<String, jsonwebtoken::errors::Error> {
        let claims = JsonClaims::new(audience, email, ttl);
        encode(&*HEADER, &claims, &EncodingKey::from_secret(key.as_bytes()))
    }

    pub fn new_auth(key: &str, email: String) -> Result<String, jsonwebtoken::errors::Error> {
        Self::new(key, Audience::Auth, email, AUTH_TOKEN_TTL)
    }

    pub fn new_web(key: &str, email: String) -> Result<String, jsonwebtoken::errors::Error> {
        Self::new(key, Audience::Web, email, WEB_TOKEN_TTL)
    }

    pub fn new_api(
        key: &str,
        email: String,
        ttl: usize,
    ) -> Result<String, jsonwebtoken::errors::Error> {
        Self::new(key, Audience::Api, email, ttl)
    }

    pub fn validate(
        key: &str,
        audience: Audience,
        token: &str,
    ) -> Result<TokenData<JsonClaims>, jsonwebtoken::errors::Error> {
        let mut validation = Validation::new(*ALGORITHM);
        validation.set_audience(&[audience]);
        validation.set_issuer(&[BENCHER_DEV]);
        validation.set_required_spec_claims(&["aud", "exp", "iss"]);
        decode(
            token,
            &DecodingKey::from_secret(key.as_bytes()),
            &validation,
        )
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonClaims {
    aud: String, // Audience
    exp: usize,  // Expiration time (as UTC timestamp)
    iat: usize,  // Issued at (as UTC timestamp)
    iss: String, // Issuer
    sub: String, // Subject (whom token refers to)
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
}

#[derive(Debug, Copy, Clone)]
pub enum Audience {
    Auth,
    Web,
    Api,
}

const AUDIENCE_AUTH: &str = "auth";
const AUDIENCE_WEB: &str = "web";
const AUDIENCE_API: &str = "api";

impl ToString for Audience {
    fn to_string(&self) -> String {
        match self {
            Self::Auth => AUDIENCE_AUTH.into(),
            Self::Web => AUDIENCE_WEB.into(),
            Self::Api => AUDIENCE_API.into(),
        }
    }
}

impl Into<String> for Audience {
    fn into(self) -> String {
        self.to_string()
    }
}
