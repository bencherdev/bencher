use bencher_valid::{NonEmpty, Slug, Url};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod alert;
pub mod benchmark;
pub mod branch;
pub mod metric;
pub mod metric_kind;
pub mod perf;
pub mod report;
pub mod result;
pub mod testbed;
pub mod threshold;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewProject {
    pub name: NonEmpty,
    pub slug: Option<Slug>,
    pub url: Option<Url>,
    pub visibility: Option<JsonVisibility>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonProjects {
    pub public: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonProject {
    pub uuid: Uuid,
    pub organization: Uuid,
    pub name: NonEmpty,
    pub slug: Slug,
    pub url: Option<Url>,
    pub visibility: JsonVisibility,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum JsonVisibility {
    #[default]
    Public,
    #[cfg(feature = "plus")]
    Private,
}

impl JsonVisibility {
    pub fn is_public(&self) -> bool {
        matches!(self, Self::Public)
    }
}
