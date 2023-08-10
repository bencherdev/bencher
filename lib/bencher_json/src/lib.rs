pub use bencher_valid::{
    BenchmarkName, Boundary, BranchName, Email, GitHash, Jwt, NonEmpty, ResourceId, SampleSize,
    Sanitize, Secret, Slug, Url, UserName, ValidError, MAX_LEN,
};
#[cfg(feature = "plus")]
pub use bencher_valid::{
    CardBrand, CardCvc, CardNumber, ExpirationMonth, ExpirationYear, PlanLevel, PlanStatus,
};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub mod big_int;
pub mod organization;
pub mod pagination;
pub mod project;
pub mod system;
pub mod urlencoded;
pub mod user;

#[cfg(feature = "plus")]
pub use organization::{metered::JsonPlan, usage::JsonUsage};

pub use big_int::BigInt;
pub use organization::{
    member::{JsonMember, JsonMembers},
    JsonNewOrganization, JsonOrganization, JsonOrganizations,
};
pub use pagination::{JsonDirection, JsonPagination};
pub use project::{
    alert::{JsonAlert, JsonAlertStats, JsonAlerts},
    benchmark::{JsonBenchmark, JsonBenchmarks},
    branch::{JsonBranch, JsonBranches, JsonNewBranch},
    metric::JsonMetric,
    metric_kind::{JsonMetricKind, JsonMetricKinds, JsonNewMetricKind},
    perf::{JsonPerf, JsonPerfQuery},
    report::{JsonNewReport, JsonReport, JsonReports},
    testbed::{JsonNewTestbed, JsonTestbed, JsonTestbeds},
    threshold::{JsonNewThreshold, JsonStatistic, JsonThreshold, JsonThresholds},
    JsonNewProject, JsonProject, JsonProjects,
};
pub use system::{
    auth::{JsonAuthToken, JsonAuthUser, JsonLogin, JsonSignup},
    backup::JsonBackup,
    config::JsonConfig,
    endpoint::JsonEndpoint,
    ping::JsonPing,
    restart::JsonRestart,
    version::JsonApiVersion,
};
pub use user::{
    token::{JsonNewToken, JsonToken, JsonTokens},
    JsonUser,
};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonEmpty {}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonAllowed {
    pub allowed: bool,
}

pub fn sanitize_json<T>(json: &T) -> serde_json::Value
where
    T: Clone + Serialize + Sanitize,
{
    if cfg!(debug_assertions) {
        serde_json::json!(json)
    } else {
        let mut sanitized = json.clone();
        sanitized.sanitize();
        serde_json::json!(sanitized)
    }
}

#[macro_export]
macro_rules! from_vec {
    ($list:ty[$single:ty]) => {
        impl From<Vec<$single>> for $list {
            fn from(vector: Vec<$single>) -> Self {
                Self(vector)
            }
        }

        impl FromIterator<$single> for $list {
            fn from_iter<I: IntoIterator<Item = $single>>(iter: I) -> Self {
                Self(iter.into_iter().collect())
            }
        }

        impl $list {
            pub fn into_inner(self) -> Vec<$single> {
                self.0
            }
        }
    };
}
