use base64::{
    engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig},
    Engine,
};
use derive_more::Display;
use once_cell::sync::Lazy;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::{fmt, str::FromStr};
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer, Serialize,
};

use crate::ValidError;

// Valid until 2159-12-06T18:53:44Z
pub const TEST_BENCHER_API_TOKEN_STR: &str = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJhdWQiOiJhcGlfa2V5IiwiZXhwIjo1OTkzNjM2MDI0LCJpYXQiOjE2OTg2Njg3MjksImlzcyI6Imh0dHA6Ly9sb2NhbGhvc3Q6MzAwMC8iLCJzdWIiOiJtdXJpZWwuYmFnZ2VAbm93aGVyZS5jb20iLCJvcmciOm51bGx9.t3t23mlgKYZmUt7-PbRWLqXlCTt6Ydh8TRE8KiSGQi4";

#[allow(clippy::expect_used)]
pub static TEST_BENCHER_API_TOKEN: Lazy<Jwt> = Lazy::new(|| {
    TEST_BENCHER_API_TOKEN_STR
        .parse()
        .expect("Invalid test JWT")
});

#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Eq, PartialEq, Hash, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
pub struct Jwt(String);

#[cfg(feature = "db")]
crate::typed_string!(Jwt);

impl FromStr for Jwt {
    type Err = ValidError;

    fn from_str(jwt: &str) -> Result<Self, Self::Err> {
        if is_valid_jwt(jwt) {
            Ok(Self(jwt.into()))
        } else {
            Err(ValidError::Jwt(jwt.into()))
        }
    }
}

impl AsRef<str> for Jwt {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<Jwt> for String {
    fn from(jwt: Jwt) -> Self {
        jwt.0
    }
}

impl Jwt {
    /// Create a valid test token
    pub fn test_token() -> Self {
        TEST_BENCHER_API_TOKEN.clone()
    }
}

impl<'de> Deserialize<'de> for Jwt {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(JwtVisitor)
    }
}

struct JwtVisitor;

impl Visitor<'_> for JwtVisitor {
    type Value = Jwt;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a valid jwt")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value.parse().map_err(E::custom)
    }
}

/// Takes the result of a rsplit and ensure we only get 2 parts
/// with a length greater than zero. Otherwise, return false.
macro_rules! expect_two {
    ($iter:expr) => {{
        let mut i = $iter;
        match (i.next(), i.next(), i.next()) {
            (Some(first), Some(second), None) if !first.is_empty() && !second.is_empty() => {
                (first, second)
            },
            _ => return false,
        }
    }};
}

// Based on
// https://github.com/validatorjs/validator.js/blob/63b61629187a732c3b3c8d89fe4cacad890cad99/src/lib/isJWT.js
// https://github.com/Keats/jsonwebtoken/blob/v8.1.1/src/decoding.rs#L167
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_jwt(jwt: &str) -> bool {
    let (signature, message) = expect_two!(jwt.rsplitn(2, '.'));
    let (payload, header) = expect_two!(message.rsplitn(2, '.'));

    // A URL safe encoding that does not have trailing `=` characters
    let url_safe = GeneralPurpose::new(
        &base64::alphabet::URL_SAFE,
        GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::RequireNone),
    );

    url_safe.decode(header).is_ok()
        && url_safe.decode(payload).is_ok()
        && url_safe.decode(signature).is_ok()
}

#[cfg(test)]
mod test {
    use crate::Jwt;

    use super::is_valid_jwt;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_jwt() {
        const HEADER: &str = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9";
        const PAYLOAD: &str = "eyJhdWQiOiJhdXRoIiwiZXhwIjoxNjY5Mjk5NjExLCJpYXQiOjE2NjkyOTc4MTEsImlzcyI6ImJlbmNoZXIuZGV2Iiwic3ViIjoiYUBhLmNvIiwib3JnIjpudWxsfQ";
        const SIGNATURE: &str = "jJmb_nCVJYLD5InaIxsQfS7x87fUsnCYpQK9SrWrKTc";

        assert_eq!(
            true,
            is_valid_jwt(&format!("{HEADER}.{PAYLOAD}.{SIGNATURE}"))
        );

        assert_eq!(false, is_valid_jwt(""));
        assert_eq!(false, is_valid_jwt(&format!(".{PAYLOAD}.{SIGNATURE}")));
        assert_eq!(false, is_valid_jwt(&format!("{HEADER}..{SIGNATURE}")));
        assert_eq!(false, is_valid_jwt(&format!("{HEADER}.{PAYLOAD}.")));
        assert_eq!(false, is_valid_jwt(&format!("{HEADER}..")));
        assert_eq!(false, is_valid_jwt(&format!(".{PAYLOAD}.")));
        assert_eq!(false, is_valid_jwt(&format!("..{SIGNATURE}")));

        assert_eq!(
            false,
            is_valid_jwt(&format!(" {HEADER}.{PAYLOAD}.{SIGNATURE}"))
        );
        assert_eq!(
            false,
            is_valid_jwt(&format!("{HEADER}.{PAYLOAD}.{SIGNATURE} "))
        );
        assert_eq!(false, is_valid_jwt(&format!("{HEADER}.{PAYLOAD}")));
        assert_eq!(false, is_valid_jwt(&format!("{HEADER}.")));
        assert_eq!(false, is_valid_jwt(&format!("{PAYLOAD}.{SIGNATURE}")));
        assert_eq!(false, is_valid_jwt(&format!(".{SIGNATURE}")));
        assert_eq!(false, is_valid_jwt(&format!("bad.{PAYLOAD}.{SIGNATURE}")));
        assert_eq!(false, is_valid_jwt(&format!("{HEADER}.bad.{SIGNATURE}")));
        assert_eq!(false, is_valid_jwt(&format!("{HEADER}.{PAYLOAD}.bad")));
        assert_eq!(
            false,
            is_valid_jwt(&format!(
                "{HEADER}.!Jmb_nCVJYLD5InaIxsQfS7x87fUsnCYpQK9SrWrKTc.{SIGNATURE}"
            ))
        );
    }

    #[test]
    fn test_jwt_test_token() {
        assert_eq!(true, is_valid_jwt(Jwt::test_token().as_ref()));
    }
}
