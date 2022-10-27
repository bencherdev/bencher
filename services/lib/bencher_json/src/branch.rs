#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewBranch {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonBranch {
    pub uuid: Uuid,
    pub project: Uuid,
    pub name: String,
    pub slug: String,
}
