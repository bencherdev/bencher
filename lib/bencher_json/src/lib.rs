pub use bencher_valid::{
    BenchmarkName, Boundary, BranchName, DateTime, DateTimeMillis, Email, GitHash, Jwt, NameId,
    NonEmpty, ResourceId, SampleSize, Sanitize, Secret, Slug, Url, UserName, ValidError, Window,
};
#[cfg(feature = "plus")]
pub use bencher_valid::{
    CardBrand, CardCvc, CardNumber, Entitlements, ExpirationMonth, ExpirationYear, LastFour,
    LicensedPlanId, MeteredPlanId, PlanLevel, PlanStatus,
};
use once_cell::sync::Lazy;
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
    measure::{JsonMeasure, JsonMeasures, JsonNewMeasure, MeasureUuid},
    metric::{JsonMetric, JsonMetricsMap, JsonResultsMap, Measure, MetricUuid},
    perf::{JsonPerf, JsonPerfQuery, PerfUuid},
    report::{JsonNewReport, JsonReport, JsonReports, ReportUuid},
    testbed::{JsonNewTestbed, JsonTestbed, JsonTestbeds, TestbedUuid},
    threshold::{
        JsonNewThreshold, JsonStatistic, JsonThreshold, JsonThresholds, StatisticUuid,
        ThresholdUuid,
    },
    JsonNewProject, JsonProject, JsonProjects, ProjectUuid,
};
#[cfg(feature = "plus")]
pub use system::server::{JsonServer, JsonServerStats, ServerUuid};
pub use system::{
    auth::{JsonAuth, JsonAuthToken, JsonAuthUser, JsonLogin, JsonSignup},
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

#[cfg(debug_assertions)]
pub const BENCHER_URL_STR: &str = "http://localhost:3000";
#[cfg(not(debug_assertions))]
pub const BENCHER_URL_STR: &str = PROD_BENCHER_URL_STR;

pub const PROD_BENCHER_URL_STR: &str = "https://bencher.dev";
pub const DEVEL_BENCHER_URL_STR: &str = "https://devel--bencher.netlify.app";

#[allow(clippy::panic)]
pub static BENCHER_URL: Lazy<url::Url> = Lazy::new(|| {
    BENCHER_URL_STR
        .parse()
        .unwrap_or_else(|e| panic!("Failed to parse endpoint \"{BENCHER_URL_STR}\": {e}"))
});
#[allow(clippy::panic)]
pub static DEVEL_BENCHER_URL: Lazy<url::Url> = Lazy::new(|| {
    DEVEL_BENCHER_URL_STR
        .parse()
        .unwrap_or_else(|e| panic!("Failed to parse endpoint \"{DEVEL_BENCHER_URL_STR}\": {e}"))
});

#[cfg(debug_assertions)]
pub const BENCHER_API_URL_STR: &str = "http://localhost:61016";
#[cfg(not(debug_assertions))]
pub const BENCHER_API_URL_STR: &str = PROD_BENCHER_API_URL_STR;

pub const PROD_BENCHER_API_URL_STR: &str = "https://api.bencher.dev";
pub const DEVEL_BENCHER_API_URL_STR: &str = "https://bencher-api-dev.fly.dev";

#[allow(clippy::panic)]
pub static BENCHER_API_URL: Lazy<url::Url> = Lazy::new(|| {
    BENCHER_API_URL_STR
        .parse()
        .unwrap_or_else(|e| panic!("Failed to parse endpoint \"{BENCHER_API_URL_STR}\": {e}"))
});
#[allow(clippy::panic)]
pub static DEVEL_BENCHER_API_URL: Lazy<url::Url> = Lazy::new(|| {
    DEVEL_BENCHER_API_URL_STR
        .parse()
        .unwrap_or_else(|e| panic!("Failed to parse endpoint \"{DEVEL_BENCHER_API_URL_STR}\": {e}"))
});

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonAny {}

/// A pre-`v1.0` future proof way to check the result of most create and update operations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonCreated {
    pub created: DateTime,
}

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
