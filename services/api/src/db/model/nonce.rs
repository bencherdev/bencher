use std::sync::Arc;

use bencher_json::{
    auth::JsonNonce,
    JsonLogin,
    JsonUser,
};
use chrono::Utc;
use diesel::{
    QueryDsl,
    RunQueryDsl,
};
use dropshot::{
    endpoint,
    HttpError,
    HttpResponseAccepted,
    HttpResponseHeaders,
    HttpResponseOk,
    Path,
    RequestContext,
    TypedBody,
};
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
use schemars::JsonSchema;
use serde::{
    Deserialize,
    Serialize,
};

use crate::{
    db::{
        model::user::QueryUser,
        schema,
    },
    diesel::ExpressionMethods,
    util::{
        cors::get_cors,
        headers::CorsHeaders,
        http_error,
        Context,
    },
};

const BENCHER_DEV: &str = "bencher.dev";
// 15 minutes * 60 seconds / minute
const TOKEN_TTL: usize = 15 * 60;

#[derive(Debug, Serialize, Deserialize)]
struct NonceClaims {
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
        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(key.as_bytes()),
        )
    }

    pub fn validate(
        key: &str,
        email: String,
        token: &str,
    ) -> Result<TokenData<Self>, jsonwebtoken::errors::Error> {
        let mut validation = Validation::new(Algorithm::default());
        validation.set_audience(&[BENCHER_DEV]);
        validation.set_issuer(&[BENCHER_DEV]);
        validation.sub = Some(email);
        validation.set_required_spec_claims(&["exp", "aud", "iss", "sub"]);
        decode(
            token,
            &DecodingKey::from_secret(key.as_bytes()),
            &validation,
        )
    }
}
