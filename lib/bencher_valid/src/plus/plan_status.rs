use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::str::FromStr;
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::wasm_bindgen;

use serde::{Deserialize, Serialize};

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
#[derive(Debug, Display, Clone, Copy, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(try_from = "String", rename_all = "snake_case")]
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

impl TryFrom<String> for PlanStatus {
    type Error = ValidError;

    fn try_from(plan_status: String) -> Result<Self, Self::Error> {
        match plan_status.as_str() {
            ACTIVE => Ok(Self::Active),
            CANCELED => Ok(Self::Canceled),
            INCOMPLETE => Ok(Self::Incomplete),
            INCOMPLETE_EXPIRED => Ok(Self::IncompleteExpired),
            PAST_DUE => Ok(Self::PastDue),
            PAUSED => Ok(Self::Paused),
            TRIALING => Ok(Self::Trialing),
            UNPAID => Ok(Self::Unpaid),
            _ => Err(ValidError::PlanStatus(plan_status)),
        }
    }
}

impl FromStr for PlanStatus {
    type Err = ValidError;

    fn from_str(plan_status: &str) -> Result<Self, Self::Err> {
        Self::try_from(plan_status.to_owned())
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
        plan_status.as_ref().to_owned()
    }
}

impl PlanStatus {
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Active | Self::Trialing)
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
mod tests {
    use super::is_valid_plan_status;
    use pretty_assertions::assert_eq;

    #[test]
    fn plan_status() {
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
