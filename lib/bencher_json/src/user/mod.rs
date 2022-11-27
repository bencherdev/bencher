pub mod token;
use bencher_valid::UserName;

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUser {
    pub uuid: Uuid,
    pub name: UserName,
    pub slug: String,
    pub email: String,
    pub admin: bool,
    pub locked: bool,
}
