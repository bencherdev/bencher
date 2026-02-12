use std::{fmt, str::FromStr};

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::wasm_bindgen;

use crate::ValidError;

const X86_64: &str = "x86_64";
const AARCH64: &str = "aarch64";

#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(try_from = "String", into = "String")]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
pub enum Architecture {
    X86_64,
    Aarch64,
}

#[cfg(feature = "db")]
crate::typed_string!(Architecture);

impl fmt::Display for Architecture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

impl TryFrom<String> for Architecture {
    type Error = ValidError;

    fn try_from(architecture: String) -> Result<Self, Self::Error> {
        match architecture.as_str() {
            X86_64 => Ok(Self::X86_64),
            AARCH64 => Ok(Self::Aarch64),
            _ => Err(ValidError::Architecture(architecture)),
        }
    }
}

impl FromStr for Architecture {
    type Err = ValidError;

    fn from_str(architecture: &str) -> Result<Self, Self::Err> {
        Self::try_from(architecture.to_owned())
    }
}

impl AsRef<str> for Architecture {
    fn as_ref(&self) -> &str {
        match self {
            Self::X86_64 => X86_64,
            Self::Aarch64 => AARCH64,
        }
    }
}

impl From<Architecture> for String {
    fn from(architecture: Architecture) -> Self {
        architecture.as_ref().to_owned()
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
#[cfg_attr(not(any(feature = "wasm", test)), expect(dead_code))]
pub fn is_valid_architecture(architecture: &str) -> bool {
    matches!(architecture, X86_64 | AARCH64)
}

#[cfg(test)]
mod tests {
    use super::is_valid_architecture;
    use pretty_assertions::assert_eq;

    #[test]
    fn architecture_valid() {
        assert_eq!(true, is_valid_architecture("x86_64"));
        assert_eq!(true, is_valid_architecture("aarch64"));
    }

    #[test]
    fn architecture_invalid() {
        assert_eq!(false, is_valid_architecture(""));
        assert_eq!(false, is_valid_architecture("arm64"));
        assert_eq!(false, is_valid_architecture("X86_64"));
        assert_eq!(false, is_valid_architecture("Aarch64"));
        assert_eq!(false, is_valid_architecture(" x86_64"));
        assert_eq!(false, is_valid_architecture("x86_64 "));
    }

    #[test]
    fn architecture_serde_roundtrip() {
        use super::Architecture;

        let arch: Architecture = serde_json::from_str("\"x86_64\"").unwrap();
        assert_eq!(arch, Architecture::X86_64);
        let json = serde_json::to_string(&arch).unwrap();
        assert_eq!(json, "\"x86_64\"");

        let arch: Architecture = serde_json::from_str("\"aarch64\"").unwrap();
        assert_eq!(arch, Architecture::Aarch64);
        let json = serde_json::to_string(&arch).unwrap();
        assert_eq!(json, "\"aarch64\"");

        let err = serde_json::from_str::<Architecture>("\"invalid\"");
        assert!(err.is_err());
    }
}
