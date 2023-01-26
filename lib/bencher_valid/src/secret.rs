#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::{fmt, str::FromStr};
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer, Serialize,
};

use crate::{Sanitize, ValidError};

const SANITIZED_SECRET: &str = "************";

#[derive(Clone, Eq, PartialEq, Hash, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct Secret(String);

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if cfg!(debug_assertions) {
            write!(f, "{}", self.0)
        } else {
            write!(f, "{SANITIZED_SECRET}")
        }
    }
}

impl Sanitize for Secret {
    fn sanitize(&mut self) {
        self.0 = SANITIZED_SECRET.into();
    }
}

impl Default for Secret {
    fn default() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

impl FromStr for Secret {
    type Err = ValidError;

    fn from_str(secret: &str) -> Result<Self, Self::Err> {
        if is_valid_secret(secret) {
            Ok(Self(secret.into()))
        } else {
            Err(ValidError::Secret(secret.into()))
        }
    }
}

impl AsRef<str> for Secret {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<Secret> for String {
    fn from(secret: Secret) -> Self {
        secret.0
    }
}

impl<'de> Deserialize<'de> for Secret {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(SecretVisitor)
    }
}

struct SecretVisitor;

impl<'de> Visitor<'de> for SecretVisitor {
    type Value = Secret;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a non-empty string")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value.parse().map_err(E::custom)
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_secret(secret: &str) -> bool {
    !secret.is_empty()
}

#[cfg(test)]
mod test {
    use super::is_valid_secret;
    use pretty_assertions::assert_eq;

    const LEN_50_STR: &str = "01234567890123456789012345678901234567890123456789";
    const LEN_51_STR: &str = "012345678901234567890123456789012345678901234567890";

    #[test]
    fn test_secret() {
        assert_eq!(true, is_valid_secret("a"));
        assert_eq!(true, is_valid_secret("ab"));
        assert_eq!(true, is_valid_secret("abc"));
        assert_eq!(true, is_valid_secret("ABC"));
        assert_eq!(true, is_valid_secret("abc ~ABC!"));
        assert_eq!(true, is_valid_secret(LEN_50_STR));
        assert_eq!(true, is_valid_secret(LEN_51_STR));

        assert_eq!(false, is_valid_secret(""));
    }
}
