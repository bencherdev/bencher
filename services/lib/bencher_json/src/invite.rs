#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::member::JsonOrganizationRole;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonInvite {
    pub name: Option<String>,
    pub email: String,
    pub organization: Uuid,
    pub role: JsonOrganizationRole,
}
