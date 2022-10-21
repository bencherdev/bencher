#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub mod alert;
pub mod auth;
pub mod benchmark;
pub mod branch;
pub mod config;
pub mod invite;
pub mod jwt;
pub mod organization;
pub mod perf;
pub mod project;
pub mod report;
pub mod resource_id;
pub mod restart;
pub mod testbed;
pub mod threshold;
pub mod token;
pub mod user;

pub use alert::JsonAlert;
pub use auth::{JsonAuthToken, JsonLogin, JsonSignup};
pub use benchmark::JsonBenchmark;
pub use branch::{JsonBranch, JsonNewBranch};
pub use config::JsonConfig;
pub use invite::JsonInvite;
pub use organization::{JsonNewOrganization, JsonOrganization};
pub use perf::{JsonPerf, JsonPerfQuery};
pub use project::{JsonNewProject, JsonProject};
pub use report::{JsonNewReport, JsonReport};
pub use resource_id::ResourceId;
pub use restart::JsonRestart;
pub use testbed::{JsonNewTestbed, JsonTestbed};
pub use threshold::{JsonNewThreshold, JsonThreshold};
pub use token::{JsonNewToken, JsonToken};
pub use user::JsonUser;

#[derive(Debug, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonEmpty {}
