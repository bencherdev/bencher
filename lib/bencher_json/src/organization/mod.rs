use bencher_valid::{NonEmpty, Slug};
use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(feature = "plus")]
pub mod entitlements;
pub mod member;
#[cfg(feature = "plus")]
pub mod metered;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewOrganization {
    pub name: NonEmpty,
    pub slug: Option<Slug>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonOrganization {
    pub uuid: Uuid,
    pub name: NonEmpty,
    pub slug: Slug,
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
