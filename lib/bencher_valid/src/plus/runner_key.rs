#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

use crate::{Sanitize, ValidError};

pub const RUNNER_KEY_PREFIX: &str = "bencher_runner_";
const RUNNER_KEY_LENGTH: usize = RUNNER_KEY_PREFIX.len() + crate::keys::KEY_RANDOM_LEN;

const SANITIZED_RUNNER_KEY: &str = "bencher_runner_******************************";

#[typeshare::typeshare]
#[derive(Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(try_from = "String")]
pub struct RunnerKey(String);

impl RunnerKey {
    #[cfg(feature = "server")]
    pub fn generate() -> Self {
        Self(format!(
            "{RUNNER_KEY_PREFIX}{}",
            crate::keys::generate_random_body()
        ))
    }
}

impl fmt::Debug for RunnerKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Display for RunnerKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if cfg!(debug_assertions) {
            write!(f, "{}", self.0)
        } else {
            write!(f, "{SANITIZED_RUNNER_KEY}")
        }
    }
}

impl Sanitize for RunnerKey {
    fn sanitize(&mut self) {
        self.0 = SANITIZED_RUNNER_KEY.into();
    }
}

impl TryFrom<String> for RunnerKey {
    type Error = ValidError;

    fn try_from(key: String) -> Result<Self, Self::Error> {
        if is_valid_runner_key(&key) {
            Ok(Self(key))
        } else {
            Err(ValidError::RunnerKey(key))
        }
    }
}

impl FromStr for RunnerKey {
    type Err = ValidError;

    fn from_str(key: &str) -> Result<Self, Self::Err> {
        Self::try_from(key.to_owned())
    }
}

impl AsRef<str> for RunnerKey {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<RunnerKey> for String {
    fn from(key: RunnerKey) -> Self {
        key.0
    }
}

fn is_valid_runner_key(key: &str) -> bool {
    key.len() == RUNNER_KEY_LENGTH
        && key
            .strip_prefix(RUNNER_KEY_PREFIX)
            .is_some_and(crate::keys::is_valid_alphanumeric_body)
}

#[cfg(test)]
#[expect(clippy::string_slice)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn is_valid_true() {
        let valid = format!(
            "{RUNNER_KEY_PREFIX}{}",
            "A".repeat(crate::keys::KEY_RANDOM_LEN)
        );
        assert!(is_valid_runner_key(&valid));

        let mixed = format!("{RUNNER_KEY_PREFIX}aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh");
        assert!(is_valid_runner_key(&mixed));
    }

    #[test]
    fn is_valid_false() {
        assert!(!is_valid_runner_key(""));
        assert!(!is_valid_runner_key("bencher_runner_short"));
        assert!(!is_valid_runner_key(
            "wrong_prefix_aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh"
        ));
        let with_special = format!("{RUNNER_KEY_PREFIX}aB3xY9mN2pQ7rS4tU8vW1zK5jL0f!");
        assert!(!is_valid_runner_key(&with_special));
        let too_long = format!("{RUNNER_KEY_PREFIX}{}", "A".repeat(31));
        assert!(!is_valid_runner_key(&too_long));
    }

    #[test]
    fn serde_roundtrip() {
        let valid = format!("{RUNNER_KEY_PREFIX}aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh");
        let json = format!("\"{valid}\"");
        let key: RunnerKey = serde_json::from_str(&json).unwrap();
        assert_eq!(key.as_ref(), valid);
        let serialized = serde_json::to_string(&key).unwrap();
        assert_eq!(serialized, json);

        let err = serde_json::from_str::<RunnerKey>("\"invalid\"");
        assert!(err.is_err());
    }

    #[cfg(feature = "server")]
    #[test]
    fn generate_valid() {
        let key = RunnerKey::generate();
        assert!(key.as_ref().starts_with(RUNNER_KEY_PREFIX));
        assert_eq!(key.as_ref().len(), RUNNER_KEY_LENGTH);
        let random_part = &key.as_ref()[RUNNER_KEY_PREFIX.len()..];
        assert!(random_part.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[cfg(feature = "server")]
    #[test]
    fn generate_unique() {
        let k1 = RunnerKey::generate();
        let k2 = RunnerKey::generate();
        assert_ne!(k1, k2);
    }

    #[test]
    fn sanitize_output() {
        let valid = format!("{RUNNER_KEY_PREFIX}aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh");
        let mut key: RunnerKey = valid.parse().unwrap();
        key.sanitize();
        assert_eq!(key.as_ref(), SANITIZED_RUNNER_KEY);
    }
}
