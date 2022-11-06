#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewTestbed {
    pub name: String,
    pub slug: Option<String>,
    pub os_name: Option<String>,
    pub os_version: Option<String>,
    pub runtime_name: Option<String>,
    pub runtime_version: Option<String>,
    pub cpu: Option<String>,
    pub gpu: Option<String>,
    pub ram: Option<String>,
    pub disk: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonTestbed {
    pub uuid: Uuid,
    pub project: Uuid,
    pub name: String,
    pub slug: String,
    pub os_name: Option<String>,
    pub os_version: Option<String>,
    pub runtime_name: Option<String>,
    pub runtime_version: Option<String>,
    pub cpu: Option<String>,
    pub gpu: Option<String>,
    pub ram: Option<String>,
    pub disk: Option<String>,
}
