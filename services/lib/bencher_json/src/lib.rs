#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub mod alert;
pub mod auth;
pub mod benchmark;
pub mod branch;
pub mod params;
pub mod perf;
pub mod project;
pub mod report;
pub mod testbed;
pub mod threshold;
pub mod jwt;

pub use auth::{JsonAuthToken, JsonLogin, JsonSignup, JsonUser};
pub use benchmark::JsonBenchmark;
pub use branch::{JsonBranch, JsonNewBranch};
pub use params::ResourceId;
pub use perf::{JsonPerf, JsonPerfQuery};
pub use project::{JsonNewProject, JsonProject};
pub use report::{JsonNewReport, JsonReport};
pub use testbed::{JsonNewTestbed, JsonTestbed};
pub use threshold::{JsonNewThreshold, JsonThreshold};

#[derive(Debug, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonEmpty {}
