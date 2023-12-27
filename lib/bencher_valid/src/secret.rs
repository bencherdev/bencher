#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::{fmt, str::FromStr};
use uuid::Uuid;

use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer, Serialize,
};

use crate::{Sanitize, ValidError};

const SANITIZED_SECRET: &str = "************";

#[typeshare::typeshare]
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

impl FromStr for Secret {
    type Err = ValidError;

    fn from_str(secret: &str) -> Result<Self, Self::Err> {
        // Unlike `NonEmpty`, `Secret` is allowed to have surrounding whitespace.
        // This is to accommodate keys with newlines at the end.
        if secret.is_empty() {
            Err(ValidError::Secret(secret.into()))
        } else {
            Ok(Self(secret.into()))
        }
    }
}

impl AsRef<str> for Secret {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<Uuid> for Secret {
    fn from(uuid: Uuid) -> Self {
        Self(uuid.to_string())
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

impl Visitor<'_> for SecretVisitor {
    type Value = Secret;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a non-empty secret string")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value.parse().map_err(E::custom)
    }
}
