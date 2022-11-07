#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewMetricKind {
    pub name: String,
    pub slug: Option<String>,
    pub units: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonMetricKind {
    pub uuid: Uuid,
    pub project: Uuid,
    pub name: String,
    pub slug: String,
    pub units: Option<String>,
}
