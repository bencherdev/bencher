//! What the console shows about its reader beyond a project, served in one request.

use bencher_valid::{BmfVersion, DateTimeMillis, ResourceName, UserName};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{UserSlug, UserUuid};
use crate::{
    OrganizationUuid, ProjectSlug, ProjectUuid, project::console_project::JsonConsoleOrganization,
};

#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleUser {
    pub uuid: UserUuid,
    pub name: UserName,
    pub slug: UserSlug,
    pub organizations: Vec<JsonConsoleOrganization>,
    /// The projects of those organizations.
    pub projects: Vec<JsonConsoleUserProject>,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleUserProject {
    pub uuid: ProjectUuid,
    pub name: ResourceName,
    pub slug: ProjectSlug,
    pub organization: OrganizationUuid,
    pub bmf_version: BmfVersion,
    /// When its newest report ended, or null with no reports.
    pub last_report: Option<DateTimeMillis>,
    /// The active alerts its Alerts list shows by default.
    pub active_alerts: u32,
}
