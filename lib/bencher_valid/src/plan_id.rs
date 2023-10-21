use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::ValidError;

// Metered
#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
pub struct MeteredPlanId(String);

#[cfg(feature = "db")]
crate::typed_string!(MeteredPlanId);

impl FromStr for MeteredPlanId {
    type Err = ValidError;

    fn from_str(plan_id: &str) -> Result<Self, Self::Err> {
        Ok(Self(plan_id.to_owned()))
    }
}

impl AsRef<str> for MeteredPlanId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<MeteredPlanId> for String {
    fn from(plan_id: MeteredPlanId) -> Self {
        plan_id.0
    }
}

// License
#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
pub struct LicensedPlanId(String);

#[cfg(feature = "db")]
crate::typed_string!(LicensedPlanId);

impl FromStr for LicensedPlanId {
    type Err = ValidError;

    fn from_str(plan_id: &str) -> Result<Self, Self::Err> {
        Ok(Self(plan_id.to_owned()))
    }
}

impl AsRef<str> for LicensedPlanId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<LicensedPlanId> for String {
    fn from(plan_id: LicensedPlanId) -> Self {
        plan_id.0
    }
}
