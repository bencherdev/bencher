//! What the console's shell shows for a project, served in one request.

use bencher_valid::ResourceName;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{JsonProject, OrganizationSlug, OrganizationUuid};

#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleProject {
    pub project: JsonProject,
    pub organization: JsonConsoleOrganization,
    /// What the signed in reader may do with the project.
    pub permissions: JsonConsolePermissions,
    /// The active alerts the Alerts list shows by default.
    pub active_alerts: u32,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleOrganization {
    pub uuid: OrganizationUuid,
    pub name: ResourceName,
    pub slug: OrganizationSlug,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each permission is granted independently"
)]
pub struct JsonConsolePermissions {
    pub view: bool,
    pub create: bool,
    pub edit: bool,
    pub delete: bool,
    pub manage: bool,
}
