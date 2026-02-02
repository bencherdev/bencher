#![cfg(feature = "plus")]

use bencher_valid::Sanitize;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub mod cloud;
pub mod github;
pub mod google;
pub mod litestream;
pub mod rate_limiting;
pub mod registry;
pub mod stats;

pub use cloud::JsonCloud;
pub use github::JsonGitHub;
pub use google::JsonGoogle;
pub use litestream::JsonLitestream;
pub use rate_limiting::JsonRateLimiting;
pub use registry::JsonRegistry;
pub use stats::JsonStats;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPlus {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limiting: Option<JsonRateLimiting>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github: Option<JsonGitHub>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub google: Option<JsonGoogle>,
    #[serde(alias = "disaster_recovery", skip_serializing_if = "Option::is_none")]
    pub litestream: Option<JsonLitestream>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<JsonStats>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cloud: Option<JsonCloud>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registry: Option<JsonRegistry>,
}

impl Sanitize for JsonPlus {
    fn sanitize(&mut self) {
        self.github.sanitize();
        self.litestream.sanitize();
        self.cloud.sanitize();
        self.registry.sanitize();
    }
}
