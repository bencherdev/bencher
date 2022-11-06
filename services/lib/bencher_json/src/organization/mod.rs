use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod member;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewOrganization {
    pub name: String,
    pub slug: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonOrganization {
    pub uuid: Uuid,
    pub name: String,
    pub slug: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Display)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum JsonOrganizationPermission {
    #[display(fmt = "view")]
    View,
    #[display(fmt = "create")]
    Create,
    #[display(fmt = "edit")]
    Edit,
    #[display(fmt = "delete")]
    Delete,
    #[display(fmt = "manage")]
    Manage,
    #[display(fmt = "view_role")]
    ViewRole,
    #[display(fmt = "create_role")]
    CreateRole,
    #[display(fmt = "edit_role")]
    EditRole,
    #[display(fmt = "delete_role")]
    DeleteRole,
}
