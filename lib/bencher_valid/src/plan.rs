use derive_more::Display;
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

const FREE: &str = "free";
const TEAM: &str = "team";
const ENTERPRISE: &str = "enterprise";

#[derive(Debug, Display, Clone, Copy, Default, Eq, PartialEq, Hash, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum Plan {
    #[default]
    Free,
    Team,
    Enterprise,
}

impl FromStr for Plan {
    type Err = ValidError;

    fn from_str(plan: &str) -> Result<Self, Self::Err> {
        if is_valid_plan(plan) {
            return Ok(match plan {
                FREE => Self::Free,
                TEAM => Self::Team,
                ENTERPRISE => Self::Enterprise,
                _ => panic!("Invalid plan"),
            });
        }

        Err(ValidError::Plan(plan.into()))
    }
}

impl AsRef<str> for Plan {
    fn as_ref(&self) -> &str {
        match self {
            Self::Free => FREE,
            Self::Team => TEAM,
            Self::Enterprise => ENTERPRISE,
        }
    }
}

impl From<Plan> for String {
    fn from(plan: Plan) -> Self {
        plan.as_ref().to_string()
    }
}

impl<'de> Deserialize<'de> for Plan {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(PlanVisitor)
    }
}

struct PlanVisitor;

impl<'de> Visitor<'de> for PlanVisitor {
    type Value = Plan;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a pricing plan")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value.parse().map_err(E::custom)
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_plan(plan: &str) -> bool {
    matches!(plan, FREE | TEAM | ENTERPRISE)
}

#[cfg(test)]
mod test {
    use super::is_valid_plan;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_plan() {
        assert_eq!(true, is_valid_plan("free"));
        assert_eq!(true, is_valid_plan("team"));
        assert_eq!(true, is_valid_plan("enterprise"));

        assert_eq!(false, is_valid_plan(""));
        assert_eq!(false, is_valid_plan("one"));
        assert_eq!(false, is_valid_plan("two"));
        assert_eq!(false, is_valid_plan(" free"));
        assert_eq!(false, is_valid_plan("free "));
        assert_eq!(false, is_valid_plan(" free "));
    }
}
