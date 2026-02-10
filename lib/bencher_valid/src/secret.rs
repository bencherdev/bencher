#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::{fmt, str::FromStr};
use uuid::Uuid;

use serde::{Deserialize, Serialize};

use crate::{Sanitize, ValidError};

pub const SANITIZED_SECRET: &str = "************";

#[typeshare::typeshare]
#[derive(Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(try_from = "String")]
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

impl TryFrom<String> for Secret {
    type Error = ValidError;

    fn try_from(secret: String) -> Result<Self, Self::Error> {
        // Unlike `NonEmpty`, `Secret` is allowed to have surrounding whitespace.
        // This is to accommodate keys with newlines at the end.
        if secret.is_empty() {
            Err(ValidError::Secret(secret))
        } else {
            Ok(Self(secret))
        }
    }
}

impl FromStr for Secret {
    type Err = ValidError;

    fn from_str(secret: &str) -> Result<Self, Self::Err> {
        Self::try_from(secret.to_owned())
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
