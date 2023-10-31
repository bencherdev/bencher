#![cfg(feature = "plus")]

use bencher_valid::DateTime;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::JsonOrganizations;

crate::typed_uuid::typed_uuid!(ServerUuid);

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonServer {
    pub uuid: ServerUuid,
    pub created: DateTime,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonServerStats {
    pub server: JsonServer,
    pub organizations: JsonOrganizations,
    // Timestamp of the stats
    pub timestamp: DateTime,
    // Number of users (created)
    pub users: JsonCohort,
    // Number of projects (created)
    pub projects: JsonCohort,
    // Number of projects (with at least one report)
    pub active_projects: JsonCohort,
    // Number of reports (created)
    pub reports: JsonCohort,
    // Number of reports per active project (created)
    pub reports_per_project: JsonCohortAvg,
    // Number of metrics (created)
    pub metrics: JsonCohort,
    // Number of metrics per report (created)
    pub metrics_per_report: JsonCohortAvg,
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
