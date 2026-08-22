use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::str::FromStr;
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::wasm_bindgen;

use serde::{Deserialize, Serialize};

use crate::{ValidError, is_valid_len};

/// The key of one benchmark parameter.
///
/// A key names an input the benchmark ran with, so it is a name like every other
/// name: non-empty, trimmed, and no longer than [`Self::MAX_LEN`].
///
/// Ordering is deliberately absent. A parameter set is canonicalized under
/// [RFC 8785][jcs], which orders keys by UTF-16 code unit, and that is not the
/// order a derived `Ord` gives. The canonicalizer owns the comparison.
///
/// [jcs]: https://www.rfc-editor.org/rfc/rfc8785
#[derive(Debug, Display, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(try_from = "String")]
pub struct ParameterKey(String);

impl ParameterKey {
    pub const MAX_LEN: usize = crate::MAX_LEN;
}

impl TryFrom<String> for ParameterKey {
    type Error = ValidError;

    fn try_from(parameter_key: String) -> Result<Self, Self::Error> {
        if is_valid_parameter_key(&parameter_key) {
            Ok(Self(parameter_key))
        } else {
            Err(ValidError::ParameterKey(parameter_key))
        }
    }
}

impl FromStr for ParameterKey {
    type Err = ValidError;

    fn from_str(parameter_key: &str) -> Result<Self, Self::Err> {
        Self::try_from(parameter_key.to_owned())
    }
}

impl AsRef<str> for ParameterKey {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<ParameterKey> for String {
    fn from(parameter_key: ParameterKey) -> Self {
        parameter_key.0
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_parameter_key(parameter_key: &str) -> bool {
    is_valid_len(parameter_key)
}

/// The string value of one benchmark parameter.
///
/// Only the string form is bound. A number or a boolean has one canonical
/// spelling that is inherently short, so a length rule over either would be a
/// rule about nothing.
#[derive(Debug, Display, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(try_from = "String")]
pub struct ParameterValue(String);

impl ParameterValue {
    pub const MAX_LEN: usize = crate::MAX_LEN;
}

impl TryFrom<String> for ParameterValue {
    type Error = ValidError;

    fn try_from(parameter_value: String) -> Result<Self, Self::Error> {
        if is_valid_parameter_value(&parameter_value) {
            Ok(Self(parameter_value))
        } else {
            Err(ValidError::ParameterValue(parameter_value))
        }
    }
}

impl FromStr for ParameterValue {
    type Err = ValidError;

    fn from_str(parameter_value: &str) -> Result<Self, Self::Err> {
        Self::try_from(parameter_value.to_owned())
    }
}

impl AsRef<str> for ParameterValue {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<ParameterValue> for String {
    fn from(parameter_value: ParameterValue) -> Self {
        parameter_value.0
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_parameter_value(parameter_value: &str) -> bool {
    is_valid_len(parameter_value)
}

#[cfg(test)]
mod tests {
    use crate::tests::{LEN_0_STR, LEN_64_STR, LEN_65_STR};

    use super::{ParameterKey, ParameterValue, is_valid_parameter_key, is_valid_parameter_value};
    use pretty_assertions::assert_eq;

    #[test]
    fn is_valid_parameter_key_true() {
        for value in ["size_mb", "op", "fsync", "threads", "1", LEN_64_STR] {
            assert_eq!(true, is_valid_parameter_key(value), "{value}");
        }
    }

    #[test]
    fn is_valid_parameter_key_false() {
        for value in [LEN_0_STR, LEN_65_STR, " ", "\r", " lead", "trail "] {
            assert_eq!(false, is_valid_parameter_key(value), "{value}");
        }
    }

    #[test]
    fn is_valid_parameter_value_true() {
        for value in ["read", "write", "sequential", LEN_64_STR] {
            assert_eq!(true, is_valid_parameter_value(value), "{value}");
        }
    }

    #[test]
    fn is_valid_parameter_value_false() {
        for value in [LEN_0_STR, LEN_65_STR, " ", "\r", " lead", "trail "] {
            assert_eq!(false, is_valid_parameter_value(value), "{value}");
        }
    }

    #[test]
    fn parameter_key_serde_roundtrip() {
        let parameter_key: ParameterKey = serde_json::from_str("\"size_mb\"").unwrap();
        assert_eq!(parameter_key.as_ref(), "size_mb");
        let json = serde_json::to_string(&parameter_key).unwrap();
        assert_eq!(json, "\"size_mb\"");

        serde_json::from_str::<ParameterKey>("\"\"").unwrap_err();
    }

    #[test]
    fn parameter_value_serde_roundtrip() {
        let parameter_value: ParameterValue = serde_json::from_str("\"read\"").unwrap();
        assert_eq!(parameter_value.as_ref(), "read");
        let json = serde_json::to_string(&parameter_value).unwrap();
        assert_eq!(json, "\"read\"");

        serde_json::from_str::<ParameterValue>("\"\"").unwrap_err();
    }
}
