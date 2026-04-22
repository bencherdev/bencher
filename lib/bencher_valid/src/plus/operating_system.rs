use std::{env::consts, fmt, str::FromStr};

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::wasm_bindgen;

use crate::ValidError;

const LINUX: &str = "linux";
const MACOS: &str = "macos";
const WINDOWS: &str = "windows";

#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(try_from = "String", into = "String", rename_all = "snake_case")]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
pub enum OperatingSystem {
    Linux,
    Macos,
    Windows,
}

#[cfg(feature = "db")]
crate::typed_string!(OperatingSystem);

impl fmt::Display for OperatingSystem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

impl TryFrom<String> for OperatingSystem {
    type Error = ValidError;

    fn try_from(os: String) -> Result<Self, Self::Error> {
        match os.as_str() {
            LINUX => Ok(Self::Linux),
            MACOS => Ok(Self::Macos),
            WINDOWS => Ok(Self::Windows),
            _ => Err(ValidError::OperatingSystem(os)),
        }
    }
}

impl OperatingSystem {
    pub fn from_host() -> Result<Self, ValidError> {
        consts::OS.parse()
    }
}

impl FromStr for OperatingSystem {
    type Err = ValidError;

    fn from_str(os: &str) -> Result<Self, Self::Err> {
        Self::try_from(os.to_owned())
    }
}

impl AsRef<str> for OperatingSystem {
    fn as_ref(&self) -> &str {
        match self {
            Self::Linux => LINUX,
            Self::Macos => MACOS,
            Self::Windows => WINDOWS,
        }
    }
}

impl From<OperatingSystem> for String {
    fn from(os: OperatingSystem) -> Self {
        os.as_ref().to_owned()
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
#[cfg_attr(not(any(feature = "wasm", test)), expect(dead_code))]
pub fn is_valid_operating_system(os: &str) -> bool {
    matches!(os, LINUX | MACOS | WINDOWS)
}

#[cfg(test)]
mod tests {
    use super::is_valid_operating_system;
    use pretty_assertions::assert_eq;

    #[test]
    fn operating_system_valid() {
        assert_eq!(true, is_valid_operating_system("linux"));
        assert_eq!(true, is_valid_operating_system("macos"));
        assert_eq!(true, is_valid_operating_system("windows"));
    }

    #[test]
    fn operating_system_invalid() {
        assert_eq!(false, is_valid_operating_system(""));
        assert_eq!(false, is_valid_operating_system("Linux"));
        assert_eq!(false, is_valid_operating_system("MacOS"));
        assert_eq!(false, is_valid_operating_system("Windows"));
        assert_eq!(false, is_valid_operating_system(" linux"));
        assert_eq!(false, is_valid_operating_system("linux "));
        assert_eq!(false, is_valid_operating_system("freebsd"));
    }

    #[test]
    fn operating_system_serde_roundtrip() {
        use super::OperatingSystem;

        let os: OperatingSystem = serde_json::from_str("\"linux\"").unwrap();
        assert_eq!(os, OperatingSystem::Linux);
        let json = serde_json::to_string(&os).unwrap();
        assert_eq!(json, "\"linux\"");

        let os: OperatingSystem = serde_json::from_str("\"macos\"").unwrap();
        assert_eq!(os, OperatingSystem::Macos);
        let json = serde_json::to_string(&os).unwrap();
        assert_eq!(json, "\"macos\"");

        let os: OperatingSystem = serde_json::from_str("\"windows\"").unwrap();
        assert_eq!(os, OperatingSystem::Windows);
        let json = serde_json::to_string(&os).unwrap();
        assert_eq!(json, "\"windows\"");

        let err = serde_json::from_str::<OperatingSystem>("\"invalid\"");
        assert!(err.is_err());
    }
}
