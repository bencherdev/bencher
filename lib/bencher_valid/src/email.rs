use derive_more::Display;
use email_address::EmailAddress;
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

#[derive(Debug, Display, Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct Email(String);

impl FromStr for Email {
    type Err = ValidError;

    fn from_str(email: &str) -> Result<Self, Self::Err> {
        if is_valid_email(email) {
            Ok(Self(email.into()))
        } else {
            Err(ValidError::Email(email.into()))
        }
    }
}

impl AsRef<str> for Email {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<Email> for String {
    fn from(email: Email) -> Self {
        email.0
    }
}

impl<'de> Deserialize<'de> for Email {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(EmailVisitor)
    }
}

struct EmailVisitor;

impl<'de> Visitor<'de> for EmailVisitor {
    type Value = Email;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a valid email")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value.parse().map_err(E::custom)
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_email(email: &str) -> bool {
    EmailAddress::is_valid(email)
}

#[cfg(test)]
mod test {
    use super::is_valid_email;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_email() {
        assert_eq!(true, is_valid_email("abc.xyz@example.com"));
        assert_eq!(true, is_valid_email("abc@example.com"));
        assert_eq!(true, is_valid_email("a@example.com"));
        assert_eq!(true, is_valid_email("abc.xyz@example.co"));
        assert_eq!(true, is_valid_email("abc@example.co"));
        assert_eq!(true, is_valid_email("a@example.co"));
        assert_eq!(true, is_valid_email("abc.xyz@example"));
        assert_eq!(true, is_valid_email("abc@example"));
        assert_eq!(true, is_valid_email("a@example"));

        assert_eq!(false, is_valid_email(""));
        assert_eq!(false, is_valid_email(" abc@example.com"));
        assert_eq!(false, is_valid_email("abc @example.com"));
        assert_eq!(false, is_valid_email("abc@example.com "));
        assert_eq!(false, is_valid_email("example.com"));
        assert_eq!(false, is_valid_email("abc.example.com"));
        assert_eq!(false, is_valid_email("abc!example.com"));
    }
}
