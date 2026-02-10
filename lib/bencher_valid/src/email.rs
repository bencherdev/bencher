use derive_more::Display;
use email_address::EmailAddress;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::str::FromStr;
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::wasm_bindgen;

use serde::{Deserialize, Serialize};

use crate::{Sanitize, ValidError, secret::SANITIZED_SECRET};

#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(try_from = "String")]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
pub struct Email(String);

#[cfg(feature = "db")]
crate::typed_string!(Email);

impl TryFrom<String> for Email {
    type Error = ValidError;

    fn try_from(email: String) -> Result<Self, Self::Error> {
        if is_valid_email(&email) {
            Ok(Self(email.to_lowercase()))
        } else {
            Err(ValidError::Email(email))
        }
    }
}

impl FromStr for Email {
    type Err = ValidError;

    fn from_str(email: &str) -> Result<Self, Self::Err> {
        Self::try_from(email.to_owned())
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

impl Sanitize for Email {
    fn sanitize(&mut self) {
        self.0 = SANITIZED_SECRET.into();
    }
}

impl Email {
    pub fn domain(&self) -> String {
        // This is safe because the `Email` struct guarantees validity
        EmailAddress::new_unchecked(self.as_ref())
            .domain()
            .to_owned()
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_email(email: &str) -> bool {
    EmailAddress::is_valid(email)
}

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;

    use super::{Email, is_valid_email};
    use pretty_assertions::assert_eq;

    use crate::tests::{LEN_64_STR, LEN_65_STR};

    #[test]
    fn is_valid_email_true() {
        for email in [
            "abc.xyz@example.com",
            "abc@example.com",
            "a@example.com",
            "abc.xyz@example.co",
            "abc@example.co",
            "a@example.co",
            "abc.xyz@example",
            "abc@example",
            "a@example",
            format!("{LEN_64_STR}@example.com").as_str(),
        ] {
            assert_eq!(true, is_valid_email(email), "{email}");
        }
    }

    #[test]
    fn is_valid_email_false() {
        for email in [
            "",
            " abc@example.com",
            "abc @example.com",
            "abc@example.com ",
            "example.com",
            "abc.example.com",
            "abc!example.com",
            format!("{LEN_65_STR}@example.com").as_str(),
        ] {
            assert_eq!(false, is_valid_email(email), "{email}");
        }
    }

    #[test]
    fn email_from_str_case_insensitive() {
        assert_eq!(
            Email::from_str("abc.xyz@example.com").unwrap(),
            Email::from_str("ABC.xYz@Example.coM").unwrap()
        );
    }
}
