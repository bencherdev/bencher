pub use bencher_valid::{
    BenchmarkName, Boundary, BranchName, Email, GitHash, Jwt, NonEmpty, ResourceId, SampleSize,
    Sanitize, Secret, Slug, Url, UserName, ValidError,
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
pub(crate) mod typed_uuid;
pub mod urlencoded;
pub mod user;

#[cfg(feature = "plus")]
pub use organization::{plan::JsonPlan, usage::JsonUsage};

pub use big_int::BigInt;
pub use organization::{
    member::{JsonMember, JsonMembers},
    JsonNewOrganization, JsonOrganization, JsonOrganizations, OrganizationUuid,
};
pub use pagination::{JsonDirection, JsonPagination};
pub use project::{
    alert::{AlertUuid, JsonAlert, JsonAlertStats, JsonAlerts},
    benchmark::{BenchmarkUuid, JsonBenchmark, JsonBenchmarks},
    boundary::{BoundaryUuid, JsonBoundaries, JsonBoundary},
    branch::{BranchUuid, JsonBranch, JsonBranches, JsonNewBranch, VersionUuid},
    metric::{JsonMetric, MetricUuid},
    metric_kind::{JsonMetricKind, JsonMetricKinds, JsonNewMetricKind, MetricKindUuid},
    perf::{JsonPerf, JsonPerfQuery, PerfUuid},
    report::{JsonNewReport, JsonReport, JsonReports, ReportUuid},
    testbed::{JsonNewTestbed, JsonTestbed, JsonTestbeds, TestbedUuid},
    threshold::{
        JsonNewThreshold, JsonStatistic, JsonThreshold, JsonThresholds, StatisticUuid,
        ThresholdUuid,
    },
    JsonNewProject, JsonProject, JsonProjects, ProjectUuid,
};
pub use system::{
    auth::{JsonAuthToken, JsonAuthUser, JsonLogin, JsonSignup},
    backup::JsonBackup,
    config::JsonConfig,
    endpoint::JsonEndpoint,
    ping::JsonPing,
    restart::JsonRestart,
    spec::JsonSpec,
    version::JsonApiVersion,
};
pub use user::{
    token::{JsonNewToken, JsonToken, JsonTokens, TokenUuid},
    JsonUser, UserUuid,
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
