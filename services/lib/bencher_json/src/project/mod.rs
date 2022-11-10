#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

pub mod alert;
pub mod benchmark;
pub mod branch;
pub mod metric;
pub mod perf;
pub mod report;
pub mod testbed;
pub mod threshold;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewProject {
    pub name: String,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub url: Option<Url>,
    #[serde(default)]
    pub public: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonProject {
    pub uuid: Uuid,
    pub organization: Uuid,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub url: Option<Url>,
    pub public: bool,
}
