use bencher_valid::{DateTime, ProjectKey, ResourceName};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{ProjectUuid, UserUuid};

crate::typed_uuid::typed_uuid!(ProjectKeyUuid);

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewProjectKey {
    /// The name of the project key.
    /// Maximum length is 64 characters.
    pub name: ResourceName,
    /// The time-to-live (TTL) for the key in seconds.
    /// If not provided, the key will not expire for over 128 years.
    pub ttl: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonProjectKeys(pub Vec<JsonProjectKey>);

crate::from_vec!(JsonProjectKeys[JsonProjectKey]);

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonProjectKey {
    pub uuid: ProjectKeyUuid,
    pub project: ProjectUuid,
    pub creator: Option<UserUuid>,
    pub name: ResourceName,
    pub creation: DateTime,
    pub expiration: DateTime,
    pub last_used_at: Option<DateTime>,
    /// The time at which the key was revoked, if any.
    /// `None` means the key is active.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revoked: Option<DateTime>,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonProjectKeyCreated {
    pub uuid: ProjectKeyUuid,
    pub project: ProjectUuid,
    pub name: ResourceName,
    /// The plaintext project key. Only returned once, at creation.
    pub key: ProjectKey,
    pub creation: DateTime,
    pub expiration: DateTime,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUpdateProjectKey {
    /// The new name of the project key.
    /// Maximum length is 64 characters.
    pub name: Option<ResourceName>,
}
