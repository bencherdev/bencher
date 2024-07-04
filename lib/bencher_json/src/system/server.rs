#![cfg(feature = "plus")]

use bencher_valid::{DateTime, ResourceName};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{JsonOrganizations, JsonUsers, ProjectUuid};

crate::typed_uuid::typed_uuid!(ServerUuid);

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonServer {
    pub uuid: ServerUuid,
    pub created: DateTime,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonServerStats {
    pub server: JsonServer,
    // Timestamp of the stats
    pub timestamp: DateTime,
    // Server organizations
    pub organizations: Option<JsonOrganizations>,
    // Server admins
    pub admins: Option<JsonUsers>,
    // Number of users (created)
    pub users: Option<JsonCohort>,
    // Number of projects (created)
    pub projects: Option<JsonCohort>,
    // Number of projects (with at least one report)
    pub active_projects: Option<JsonCohort>,
    // Number of reports (created)
    pub reports: Option<JsonCohort>,
    // Number of reports per active project (created)
    pub reports_per_project: Option<JsonCohortAvg>,
    // Number of metrics (created)
    pub metrics: Option<JsonCohort>,
    // Number of metrics per report (created)
    pub metrics_per_report: Option<JsonCohortAvg>,
    // Top 10 projects
    pub top_projects: Option<JsonTopCohort>,
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

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonTopCohort {
    pub week: JsonTopProjects,
    pub month: JsonTopProjects,
    pub total: JsonTopProjects,
}

pub type JsonTopProjects = Vec<JsonTopProject>;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonTopProject {
    pub name: ResourceName,
    pub uuid: ProjectUuid,
    pub metrics: u64,
    pub percentage: f64,
}
