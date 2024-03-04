use bencher_valid::{DateTime, Jwt, ResourceName};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::UserUuid;

crate::typed_uuid::typed_uuid!(TokenUuid);

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewToken {
    /// The name of the token.
    /// Maximum length is 64 characters.
    pub name: ResourceName,
    /// The time-to-live (TTL) for the token in seconds.
    /// If not provided, the token will not expire for over 128 years.
    pub ttl: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonTokens(pub Vec<JsonToken>);

crate::from_vec!(JsonTokens[JsonToken]);

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonToken {
    pub uuid: TokenUuid,
    pub user: UserUuid,
    pub name: ResourceName,
    pub token: Jwt,
    pub creation: DateTime,
    pub expiration: DateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUpdateToken {
    /// The new name of the token.
    /// Maximum length is 64 characters.
    pub name: Option<ResourceName>,
}
