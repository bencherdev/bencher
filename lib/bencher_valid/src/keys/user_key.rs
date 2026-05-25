#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

use crate::{Sanitize, ValidError};

pub const USER_KEY_PREFIX: &str = "bencher_user_";
const USER_KEY_LENGTH: usize = USER_KEY_PREFIX.len() + super::KEY_RANDOM_LEN;

const SANITIZED_USER_KEY: &str = "bencher_user_******************************";

#[typeshare::typeshare]
#[derive(Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(try_from = "String")]
pub struct UserKey(String);

impl UserKey {
    #[cfg(feature = "server")]
    pub fn generate() -> Self {
        Self(format!(
            "{USER_KEY_PREFIX}{}",
            super::generate_random_body()
        ))
    }
}

impl fmt::Debug for UserKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Display for UserKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if cfg!(debug_assertions) {
            write!(f, "{}", self.0)
        } else {
            write!(f, "{SANITIZED_USER_KEY}")
        }
    }
}

impl Sanitize for UserKey {
    fn sanitize(&mut self) {
        self.0 = SANITIZED_USER_KEY.into();
    }
}

impl TryFrom<String> for UserKey {
    type Error = ValidError;

    fn try_from(key: String) -> Result<Self, Self::Error> {
        if is_valid_user_key(&key) {
            Ok(Self(key))
        } else {
            Err(ValidError::UserKey(key))
        }
    }
}

impl FromStr for UserKey {
    type Err = ValidError;

    fn from_str(key: &str) -> Result<Self, Self::Err> {
        Self::try_from(key.to_owned())
    }
}

impl AsRef<str> for UserKey {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<UserKey> for String {
    fn from(key: UserKey) -> Self {
        key.0
    }
}

fn is_valid_user_key(key: &str) -> bool {
    key.len() == USER_KEY_LENGTH
        && key
            .strip_prefix(USER_KEY_PREFIX)
            .is_some_and(super::is_valid_alphanumeric_body)
}

#[cfg(test)]
#[expect(clippy::string_slice, reason = "test strings have known ASCII content")]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn is_valid_true() {
        let valid = format!(
            "{USER_KEY_PREFIX}{}",
            "A".repeat(super::super::KEY_RANDOM_LEN)
        );
        assert!(is_valid_user_key(&valid));

        let mixed = format!("{USER_KEY_PREFIX}aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh");
        assert!(is_valid_user_key(&mixed));
    }

    #[test]
    fn is_valid_false() {
        assert!(!is_valid_user_key(""));
        assert!(!is_valid_user_key("bencher_user_short"));
        assert!(!is_valid_user_key(
            "wrong_prefix_aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh"
        ));
        assert!(!is_valid_user_key(
            "bencher_run_aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh"
        ));
        let with_special = format!("{USER_KEY_PREFIX}aB3xY9mN2pQ7rS4tU8vW1zK5jL0f!");
        assert!(!is_valid_user_key(&with_special));
        let too_long = format!("{USER_KEY_PREFIX}{}", "A".repeat(31));
        assert!(!is_valid_user_key(&too_long));
    }

    #[test]
    fn serde_roundtrip() {
        let valid = format!("{USER_KEY_PREFIX}aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh");
        let json = format!("\"{valid}\"");
        let key: UserKey = serde_json::from_str(&json).unwrap();
        assert_eq!(key.as_ref(), valid);
        let serialized = serde_json::to_string(&key).unwrap();
        assert_eq!(serialized, json);

        serde_json::from_str::<UserKey>("\"invalid\"").unwrap_err();
    }

    #[cfg(feature = "server")]
    #[test]
    fn generate_valid() {
        let key = UserKey::generate();
        assert!(key.as_ref().starts_with(USER_KEY_PREFIX));
        assert_eq!(key.as_ref().len(), USER_KEY_LENGTH);
        let random_part = &key.as_ref()[USER_KEY_PREFIX.len()..];
        assert!(random_part.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[cfg(feature = "server")]
    #[test]
    fn generate_unique() {
        let k1 = UserKey::generate();
        let k2 = UserKey::generate();
        assert_ne!(k1, k2);
    }

    #[test]
    fn sanitize_output() {
        let valid = format!("{USER_KEY_PREFIX}aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh");
        let mut key: UserKey = valid.parse().unwrap();
        key.sanitize();
        assert_eq!(key.as_ref(), SANITIZED_USER_KEY);
    }
}
