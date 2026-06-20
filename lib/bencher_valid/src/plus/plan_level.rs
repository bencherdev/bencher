use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::str::FromStr;
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::wasm_bindgen;

use serde::{Deserialize, Serialize};

use crate::ValidError;

const FREE: &str = "free";
const PRO: &str = "pro";
const TEAM: &str = "team";
const ENTERPRISE: &str = "enterprise";

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
    Pro,
    // Legacy self-serve paid tier, retained for grandfathered customers.
    // New self-serve signups use `Pro`.
    Team,
    Enterprise,
}

impl TryFrom<String> for PlanLevel {
    type Error = ValidError;

    fn try_from(plan_level: String) -> Result<Self, Self::Error> {
        match plan_level.as_str() {
            FREE => Ok(Self::Free),
            PRO => Ok(Self::Pro),
            TEAM => Ok(Self::Team),
            ENTERPRISE => Ok(Self::Enterprise),
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
            Self::Pro => PRO,
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
#[cfg_attr(
    not(any(feature = "wasm", test)),
    expect(dead_code, reason = "exported only for wasm and tests")
)]
pub fn is_valid_plan_level(plan_level: &str) -> bool {
    matches!(plan_level, FREE | PRO | TEAM | ENTERPRISE)
}

#[cfg(test)]
mod tests {
    use super::is_valid_plan_level;
    use pretty_assertions::assert_eq;

    #[test]
    fn plan_level() {
        assert_eq!(true, is_valid_plan_level("free"));
        assert_eq!(true, is_valid_plan_level("pro"));
        assert_eq!(true, is_valid_plan_level("team"));
        assert_eq!(true, is_valid_plan_level("enterprise"));

        // Stripe product names are no longer valid plan levels: the plan level is
        // resolved authoritatively, not by parsing a product name. These guard
        // against re-introducing that conflation.
        assert_eq!(false, is_valid_plan_level("Bencher Pro"));
        assert_eq!(false, is_valid_plan_level("Bencher Team"));
        assert_eq!(false, is_valid_plan_level("Bencher Enterprise"));
        assert_eq!(false, is_valid_plan_level("Bencher Metrics"));
        assert_eq!(false, is_valid_plan_level("Bencher Bare Metal"));

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

        let level: PlanLevel = serde_json::from_str("\"pro\"").unwrap();
        assert_eq!(level, PlanLevel::Pro);
        let json = serde_json::to_string(&level).unwrap();
        assert_eq!(json, "\"pro\"");

        serde_json::from_str::<PlanLevel>("\"invalid\"").unwrap_err();
    }
}
