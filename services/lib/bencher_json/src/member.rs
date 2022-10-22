use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonMember {
    pub uuid: Uuid,
    pub name: String,
    pub slug: String,
    pub email: String,
    pub role: JsonOrganizationRole,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, Display)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum JsonOrganizationRole {
    #[display(fmt = "member")]
    Member,
    #[display(fmt = "leader")]
    Leader,
}
