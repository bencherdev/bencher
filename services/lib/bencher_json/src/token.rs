use chrono::{DateTime, Utc};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewToken {
    pub ttl: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonToken {
    pub uuid: Uuid,
    pub user: Uuid,
    pub token: String,
    pub creation: DateTime<Utc>,
    pub expiration: DateTime<Utc>,
}
