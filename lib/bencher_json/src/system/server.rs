#![cfg(feature = "plus")]

use bencher_valid::{DateTime, ResourceName};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{JsonOrganizations, JsonUsers, ProjectUuid};

crate::typed_uuid::typed_uuid!(ServerUuid);

/// A Bencher server instance
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonServer {
    /// The server UUID
    pub uuid: ServerUuid,
    /// The date the server was created
    pub created: DateTime,
    /// The current version of the server
    pub version: Option<String>,
}

/// Bencher server stats
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonServerStats {
    /// The Bencher server instance
    pub server: JsonServer,
    /// Timestamp of the stats
    pub timestamp: DateTime,
    /// Server organizations
    pub organizations: Option<JsonOrganizations>,
    /// Server admins
    pub admins: Option<JsonUsers>,
    /// Number of users (created)
    pub users: Option<JsonCohort>,
    /// Number of projects (created)
    pub projects: Option<JsonCohort>,
    /// Number of unclaimed projects (created)
    pub projects_unclaimed: Option<JsonCohort>,
    /// Number of claimed projects (created)
    pub projects_claimed: Option<JsonCohort>,
    /// Number of active projects (with at least one report)
    pub active_projects: Option<JsonCohort>,
    /// Number of active unclaimed projects (with at least one report)
    pub active_projects_unclaimed: Option<JsonCohort>,
    /// Number of active claimed projects (with at least one report)
    pub active_projects_claimed: Option<JsonCohort>,
    /// Number of reports (created)
    pub reports: Option<JsonCohort>,
    /// Number of unclaimed reports (created)
    pub reports_unclaimed: Option<JsonCohort>,
    /// Number of claimed reports (created)
    pub reports_claimed: Option<JsonCohort>,
    /// Number of reports per active project (created)
    pub reports_per_project: Option<JsonCohortAvg>,
    /// Number of reports per active unclaimed project (created)
    pub reports_per_project_unclaimed: Option<JsonCohortAvg>,
    /// Number of reports per active claimed project (created)
    pub reports_per_project_claimed: Option<JsonCohortAvg>,
    /// Number of metrics (created)
    pub metrics: Option<JsonCohort>,
    /// Number of unclaimed metrics (created)
    pub metrics_unclaimed: Option<JsonCohort>,
    /// Number of claimed metrics (created)
    pub metrics_claimed: Option<JsonCohort>,
    /// Number of metrics per report (created)
    pub metrics_per_report: Option<JsonCohortAvg>,
    /// Number of metrics per unclaimed report (created)
    pub metrics_per_report_unclaimed: Option<JsonCohortAvg>,
    /// Number of metrics per claimed report (created)
    pub metrics_per_report_claimed: Option<JsonCohortAvg>,
    /// Top 10 projects
    pub top_projects: Option<JsonTopCohort>,
    /// Top 10 unclaimed projects
    pub top_projects_unclaimed: Option<JsonTopCohort>,
    /// Top 10 claimed projects
    pub top_projects_claimed: Option<JsonTopCohort>,
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
