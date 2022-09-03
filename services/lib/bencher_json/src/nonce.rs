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
const TOKEN_TTL: usize = 15 * 60;

lazy_static::lazy_static! {
    static ref HEADER: Header = Header::default();
    static ref ALGORITHM: Algorithm = Algorithm::default();
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct NonceClaims {
    aud: String, // Audience
    exp: usize,  // Expiration time (as UTC timestamp)
    iat: usize,  // Issued at (as UTC timestamp)
    iss: String, // Issuer
    sub: String, // Subject (whom token refers to)
}

impl NonceClaims {
    pub fn new(key: &str, email: String) -> Result<String, jsonwebtoken::errors::Error> {
        let now = Utc::now().timestamp() as usize;
        let claims = Self {
            aud: BENCHER_DEV.into(),
            exp: now + TOKEN_TTL,
            iat: now,
            iss: BENCHER_DEV.into(),
            sub: email,
        };
        encode(&*HEADER, &claims, &EncodingKey::from_secret(key.as_bytes()))
    }

    pub fn validate(
        key: &str,
        email: String,
        token: &str,
    ) -> Result<TokenData<Self>, jsonwebtoken::errors::Error> {
        let mut validation = Validation::new(*ALGORITHM);
        validation.set_audience(&[BENCHER_DEV]);
        validation.set_issuer(&[BENCHER_DEV]);
        validation.sub = Some(email);
        validation.set_required_spec_claims(&["aud", "exp", "iss", "sub"]);
        decode(
            token,
            &DecodingKey::from_secret(key.as_bytes()),
            &validation,
        )
    }
}
