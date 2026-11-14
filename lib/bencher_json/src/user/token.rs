use bencher_valid::{DateTime, ResourceName};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::UserUuid;

crate::typed_uuid::typed_uuid!(TokenUuid);

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
    pub creation: DateTime,
    pub expiration: DateTime,
    /// The time at which the token was revoked, if any.
    /// `None` means the token is active.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revoked: Option<DateTime>,
}
