#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub mod organization;
pub mod project;
pub mod resource_id;
pub mod system;
pub mod user;

pub use organization::{member::JsonMember, JsonNewOrganization, JsonOrganization};
pub use project::{
    alert::JsonAlert,
    benchmark::JsonBenchmark,
    branch::{JsonBranch, JsonNewBranch},
    metric::JsonMetric,
    metric_kind::{JsonMetricKind, JsonNewMetricKind},
    perf::{JsonPerf, JsonPerfQuery},
    report::{JsonNewReport, JsonReport},
    result::{JsonMetrics, JsonResult},
    testbed::{JsonNewTestbed, JsonTestbed},
    threshold::{JsonNewThreshold, JsonThreshold},
    JsonNewProject, JsonProject,
};
pub use resource_id::ResourceId;
pub use system::{
    auth::{JsonAuthToken, JsonLogin, JsonSignup},
    config::JsonConfig,
    jwt::JsonWebToken,
    restart::JsonRestart,
    version::JsonVersion,
};
pub use user::{
    token::{JsonNewToken, JsonToken},
    JsonUser,
};

#[derive(Debug, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonEmpty {}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonAllowed {
    pub allowed: bool,
}
