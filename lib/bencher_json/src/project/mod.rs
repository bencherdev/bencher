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
    pub public: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonProject {
    pub uuid: Uuid,
    pub organization: Uuid,
    pub name: NonEmpty,
    pub slug: Slug,
    pub url: Option<Url>,
    pub public: bool,
}
