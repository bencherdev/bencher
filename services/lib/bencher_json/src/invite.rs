#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{member::JsonOrganizationRole, ResourceId};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonInvite {
    pub name: Option<String>,
    pub email: String,
    pub organization: ResourceId,
    pub role: JsonOrganizationRole,
}
