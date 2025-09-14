#![cfg(feature = "plus")]

use bencher_valid::{DateTime, NonEmpty};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

crate::typed_uuid::typed_uuid!(SsoUuid);

#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewSso {
    pub domain: NonEmpty,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonSso {
    pub uuid: SsoUuid,
    pub domain: NonEmpty,
    pub created: DateTime,
}
