#![cfg(feature = "plus")]

use bencher_valid::DateTime;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonServerStats {
    pub timestamp: DateTime,
    pub users: JsonCohort,
    pub projects: JsonCohort,
    pub reports: JsonCohortAvg,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonCohort {
    pub week: u64,
    pub month: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonCohortAvg {
    pub week: f64,
    pub month: f64,
    pub total: f64,
}
