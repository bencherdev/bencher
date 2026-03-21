use std::{fmt, str::FromStr};

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::wasm_bindgen;

use crate::ValidError;

const FIRECRACKER: &str = "firecracker";

#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(try_from = "String", into = "String", rename_all = "snake_case")]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
pub enum Sandbox {
    Firecracker,
}

#[cfg(feature = "db")]
crate::typed_string!(Sandbox);

impl fmt::Display for Sandbox {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

impl TryFrom<String> for Sandbox {
    type Error = ValidError;

    fn try_from(sandbox: String) -> Result<Self, Self::Error> {
        match sandbox.as_str() {
            FIRECRACKER => Ok(Self::Firecracker),
            _ => Err(ValidError::Sandbox(sandbox)),
        }
    }
}

impl FromStr for Sandbox {
    type Err = ValidError;

    fn from_str(sandbox: &str) -> Result<Self, Self::Err> {
        Self::try_from(sandbox.to_owned())
    }
}

impl AsRef<str> for Sandbox {
    fn as_ref(&self) -> &str {
        match self {
            Self::Firecracker => FIRECRACKER,
        }
    }
}

impl From<Sandbox> for String {
    fn from(sandbox: Sandbox) -> Self {
        sandbox.as_ref().to_owned()
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
#[cfg_attr(not(any(feature = "wasm", test)), expect(dead_code))]
pub fn is_valid_sandbox(sandbox: &str) -> bool {
    matches!(sandbox, FIRECRACKER)
}

#[cfg(test)]
mod tests {
    use super::is_valid_sandbox;
    use pretty_assertions::assert_eq;

    #[test]
    fn sandbox_valid() {
        assert_eq!(true, is_valid_sandbox("firecracker"));
    }

    #[test]
    fn sandbox_invalid() {
        assert_eq!(false, is_valid_sandbox(""));
        assert_eq!(false, is_valid_sandbox("Firecracker"));
        assert_eq!(false, is_valid_sandbox("FIRECRACKER"));
        assert_eq!(false, is_valid_sandbox(" firecracker"));
        assert_eq!(false, is_valid_sandbox("firecracker "));
        assert_eq!(false, is_valid_sandbox("docker"));
    }

    #[test]
    fn sandbox_serde_roundtrip() {
        use super::Sandbox;

        let sandbox: Sandbox = serde_json::from_str("\"firecracker\"").unwrap();
        assert_eq!(sandbox, Sandbox::Firecracker);
        let json = serde_json::to_string(&sandbox).unwrap();
        assert_eq!(json, "\"firecracker\"");

        let err = serde_json::from_str::<Sandbox>("\"invalid\"");
        assert!(err.is_err());
    }
}
