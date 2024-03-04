pub mod token;
use bencher_valid::{Email, Slug, UserName};

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

crate::typed_uuid::typed_uuid!(UserUuid);

#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUser {
    pub uuid: UserUuid,
    pub name: UserName,
    pub slug: Slug,
    pub email: Email,
    pub admin: bool,
    pub locked: bool,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUpdateUser {
    /// The new name of the user.
    /// Maximum length is 64 characters.
    /// May only contain alphanumeric characters, non-leading or trailing spaces, and the following characters: , . - '
    pub name: Option<UserName>,
    /// The preferred new slug for the user.
    /// Maximum length is 64 characters.
    pub slug: Option<Slug>,
    /// The new email for the user.
    pub email: Option<Email>,
    /// Update whether the user is an admin.
    /// Must be an admin to update this field.
    pub admin: Option<bool>,
    /// Update whether the user is locked.
    /// Must be an admin to update this field.
    pub locked: Option<bool>,
}
