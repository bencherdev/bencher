use std::fmt;

use bencher_valid::{NonEmpty, Slug, Url};
use chrono::{DateTime, Utc};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod alert;
pub mod benchmark;
pub mod boundary;
pub mod branch;
pub mod metric;
pub mod metric_kind;
pub mod perf;
pub mod report;
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
pub struct JsonProjects(pub Vec<JsonProject>);

crate::from_vec!(JsonProjects[JsonProject]);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonProject {
    pub uuid: Uuid,
    pub organization: Uuid,
    pub name: NonEmpty,
    pub slug: Slug,
    pub url: Option<Url>,
    pub visibility: JsonVisibility,
    pub created: DateTime<Utc>,
    pub modified: DateTime<Utc>,
}

impl fmt::Display for JsonProject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUpdateProject {
    pub name: Option<NonEmpty>,
    pub slug: Option<Slug>,
    pub url: Option<Url>,
    pub visibility: Option<JsonVisibility>,
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
