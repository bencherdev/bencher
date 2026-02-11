use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::str::FromStr;
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::wasm_bindgen;

use serde::{Deserialize, Serialize};

use crate::ValidError;

const FREE: &str = "free";
const TEAM: &str = "team";
const ENTERPRISE: &str = "enterprise";
// These are the Stripe product names
const BENCHER_TEAM: &str = "Bencher Team";
const BENCHER_ENTERPRISE: &str = "Bencher Enterprise";

#[typeshare::typeshare]
#[derive(
    Debug,
    Display,
    Clone,
    Copy,
    Default,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    Serialize,
    Deserialize,
)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(try_from = "String", into = "String", rename_all = "snake_case")]
pub enum PlanLevel {
    #[default]
    Free,
    Team,
    Enterprise,
}

impl TryFrom<String> for PlanLevel {
    type Error = ValidError;

    fn try_from(plan_level: String) -> Result<Self, Self::Error> {
        match plan_level.as_str() {
            FREE => Ok(Self::Free),
            TEAM | BENCHER_TEAM => Ok(Self::Team),
            ENTERPRISE | BENCHER_ENTERPRISE => Ok(Self::Enterprise),
            _ => Err(ValidError::PlanLevel(plan_level)),
        }
    }
}

impl FromStr for PlanLevel {
    type Err = ValidError;

    fn from_str(plan_level: &str) -> Result<Self, Self::Err> {
        Self::try_from(plan_level.to_owned())
    }
}

impl AsRef<str> for PlanLevel {
    fn as_ref(&self) -> &str {
        match self {
            Self::Free => FREE,
            Self::Team => TEAM,
            Self::Enterprise => ENTERPRISE,
        }
    }
}

impl From<PlanLevel> for String {
    fn from(plan_level: PlanLevel) -> Self {
        plan_level.as_ref().to_owned()
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
#[cfg_attr(not(feature = "wasm"), expect(dead_code))]
pub fn is_valid_plan_level(plan_level: &str) -> bool {
    matches!(
        plan_level,
        FREE | TEAM | ENTERPRISE | BENCHER_TEAM | BENCHER_ENTERPRISE
    )
}

#[cfg(test)]
mod tests {
    use super::is_valid_plan_level;
    use pretty_assertions::assert_eq;

    #[test]
    fn plan_level() {
        assert_eq!(true, is_valid_plan_level("free"));
        assert_eq!(true, is_valid_plan_level("team"));
        assert_eq!(true, is_valid_plan_level("enterprise"));
        assert_eq!(true, is_valid_plan_level("Bencher Team"));
        assert_eq!(true, is_valid_plan_level("Bencher Enterprise"));

        assert_eq!(false, is_valid_plan_level(""));
        assert_eq!(false, is_valid_plan_level("one"));
        assert_eq!(false, is_valid_plan_level("two"));
        assert_eq!(false, is_valid_plan_level(" free"));
        assert_eq!(false, is_valid_plan_level("free "));
        assert_eq!(false, is_valid_plan_level(" free "));
    }

    #[test]
    fn plan_level_serde_roundtrip() {
        use super::PlanLevel;

        let level: PlanLevel = serde_json::from_str("\"team\"").unwrap();
        assert_eq!(level, PlanLevel::Team);
        let json = serde_json::to_string(&level).unwrap();
        assert_eq!(json, "\"team\"");

        let err = serde_json::from_str::<PlanLevel>("\"invalid\"");
        assert!(err.is_err());
    }
}
