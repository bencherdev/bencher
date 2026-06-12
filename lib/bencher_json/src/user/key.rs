use bencher_valid::{DateTime, ResourceName, UserKey};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::UserUuid;

crate::typed_uuid::typed_uuid!(UserKeyUuid);

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewUserKey {
    /// The name of the user key.
    /// Maximum length is 64 characters.
    pub name: ResourceName,
    /// The time-to-live (TTL) for the key in seconds.
    /// If not provided, the key will not expire for over 128 years.
    pub ttl: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUserKeys(pub Vec<JsonUserKey>);

crate::from_vec!(JsonUserKeys[JsonUserKey]);

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUserKey {
    pub uuid: UserKeyUuid,
    pub user: UserUuid,
    pub name: ResourceName,
    pub creation: DateTime,
    pub expiration: DateTime,
    /// The time at which the key was revoked, if any.
    /// `None` means the key is active.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revoked: Option<DateTime>,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUserKeyCreated {
    pub uuid: UserKeyUuid,
    pub user: UserUuid,
    pub name: ResourceName,
    /// The plaintext user key. Only returned once, at creation.
    pub key: UserKey,
    pub creation: DateTime,
    pub expiration: DateTime,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUpdateUserKey {
    /// The new name of the user key.
    /// Maximum length is 64 characters.
    pub name: Option<ResourceName>,
}
