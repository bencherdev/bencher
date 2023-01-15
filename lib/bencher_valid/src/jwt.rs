use derive_more::Display;
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

#[derive(Debug, Display, Clone, Eq, PartialEq, Hash, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct Jwt(String);

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

impl<'de> Deserialize<'de> for Jwt {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(JwtVisitor)
    }
}

struct JwtVisitor;

impl<'de> Visitor<'de> for JwtVisitor {
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

    base64::decode_config(header, base64::URL_SAFE).is_ok()
        && base64::decode_config(payload, base64::URL_SAFE).is_ok()
        && base64::decode_config(signature, base64::URL_SAFE).is_ok()
}

#[cfg(test)]
mod test {
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

        assert_eq!(false, is_valid_jwt(&format!("")));
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
}
