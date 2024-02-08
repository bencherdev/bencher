#![cfg(feature = "plus")]

use bencher_valid::Sanitize;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub mod cloud;
pub mod github;
pub mod litestream;
pub mod stats;

pub use cloud::JsonCloud;
pub use github::JsonGitHub;
pub use litestream::JsonLitestream;
pub use stats::JsonStats;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPlus {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github: Option<JsonGitHub>,
    #[serde(alias = "disaster_recovery", skip_serializing_if = "Option::is_none")]
    pub litestream: Option<JsonLitestream>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<JsonStats>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cloud: Option<JsonCloud>,
}

impl Sanitize for JsonPlus {
    fn sanitize(&mut self) {
        self.github.sanitize();
        self.litestream.sanitize();
        self.cloud.sanitize();
    }
}
