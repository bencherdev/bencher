#![cfg(feature = "plus")]

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

const ACTIVE: &str = "active";
const CANCELED: &str = "canceled";
const INCOMPLETE: &str = "incomplete";
const INCOMPLETE_EXPIRED: &str = "incomplete_expired";
const PAST_DUE: &str = "past_due";
const PAUSED: &str = "paused";
const TRIALING: &str = "trialing";
const UNPAID: &str = "unpaid";

#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Copy, Eq, PartialEq, Hash, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum PlanStatus {
    Active,
    Canceled,
    Incomplete,
    IncompleteExpired,
    PastDue,
    Paused,
    Trialing,
    Unpaid,
}

impl FromStr for PlanStatus {
    type Err = ValidError;

    fn from_str(plan_status: &str) -> Result<Self, Self::Err> {
        if is_valid_plan_status(plan_status) {
            return Ok(match plan_status {
                ACTIVE => Self::Active,
                CANCELED => Self::Canceled,
                INCOMPLETE => Self::Incomplete,
                INCOMPLETE_EXPIRED => Self::IncompleteExpired,
                PAST_DUE => Self::PastDue,
                PAUSED => Self::Paused,
                TRIALING => Self::Trialing,
                UNPAID => Self::Unpaid,
                _ => panic!("Invalid plan level"),
            });
        }

        Err(ValidError::PlanStatus(plan_status.into()))
    }
}

impl AsRef<str> for PlanStatus {
    fn as_ref(&self) -> &str {
        match self {
            Self::Active => ACTIVE,
            Self::Canceled => CANCELED,
            Self::Incomplete => INCOMPLETE,
            Self::IncompleteExpired => INCOMPLETE_EXPIRED,
            Self::PastDue => PAST_DUE,
            Self::Paused => PAUSED,
            Self::Trialing => TRIALING,
            Self::Unpaid => UNPAID,
        }
    }
}

impl From<PlanStatus> for String {
    fn from(plan_status: PlanStatus) -> Self {
        plan_status.as_ref().to_string()
    }
}

impl PlanStatus {
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Active | Self::Trialing)
    }
}

impl<'de> Deserialize<'de> for PlanStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(PlanStatusVisitor)
    }
}

struct PlanStatusVisitor;

impl<'de> Visitor<'de> for PlanStatusVisitor {
    type Value = PlanStatus;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a valid plan status")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value.parse().map_err(E::custom)
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_plan_status(plan_status: &str) -> bool {
    matches!(
        plan_status,
        ACTIVE | CANCELED | INCOMPLETE | INCOMPLETE_EXPIRED | PAST_DUE | PAUSED | TRIALING | UNPAID
    )
}

#[cfg(test)]
mod test {
    use super::is_valid_plan_status;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_plan_status() {
        assert_eq!(true, is_valid_plan_status("active"));
        assert_eq!(true, is_valid_plan_status("canceled"));
        assert_eq!(true, is_valid_plan_status("incomplete"));
        assert_eq!(true, is_valid_plan_status("incomplete_expired"));
        assert_eq!(true, is_valid_plan_status("past_due"));
        assert_eq!(true, is_valid_plan_status("paused"));
        assert_eq!(true, is_valid_plan_status("trialing"));
        assert_eq!(true, is_valid_plan_status("unpaid"));

        assert_eq!(false, is_valid_plan_status(""));
        assert_eq!(false, is_valid_plan_status("one"));
        assert_eq!(false, is_valid_plan_status("two"));
        assert_eq!(false, is_valid_plan_status(" active"));
        assert_eq!(false, is_valid_plan_status("active "));
        assert_eq!(false, is_valid_plan_status(" active "));
    }
}
