pub use bencher_valid::{
    BenchmarkName, Boundary, BranchName, CdfBoundary, DateTime, DateTimeMillis, Email, GitHash,
    IqrBoundary, Jwt, Model, ModelTest, NameId, NameIdKind, NonEmpty, PercentageBoundary,
    ResourceId, ResourceIdKind, ResourceName, SampleSize, Sanitize, Secret, Slug, Url, UserName,
    ValidError, Window,
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
    branch::{BranchUuid, JsonBranch, JsonBranches, JsonNewBranch, JsonStartPoint, VersionUuid},
    measure::{JsonMeasure, JsonMeasures, JsonNewMeasure, MeasureUuid},
    metric::{JsonMetric, JsonMetricsMap, JsonResultsMap, MeasureNameId, MetricUuid},
    model::{JsonModel, ModelUuid},
    perf::{JsonPerf, JsonPerfQuery, PerfUuid},
    report::{JsonNewReport, JsonReport, JsonReports, ReportUuid},
    testbed::{JsonNewTestbed, JsonTestbed, JsonTestbeds, TestbedUuid},
    threshold::{JsonNewThreshold, JsonThreshold, JsonThresholds, ThresholdUuid},
    JsonNewProject, JsonProject, JsonProjects, ProjectUuid,
};
#[cfg(feature = "plus")]
pub use system::{
    auth::JsonOAuth,
    config::JsonConsole,
    payment::JsonPayment,
    server::{JsonServer, JsonServerStats, ServerUuid},
};
pub use system::{
    auth::{JsonAccept, JsonAuthAck, JsonAuthUser, JsonConfirm, JsonLogin, JsonSignup},
    backup::{JsonBackup, JsonBackupCreated},
    config::JsonConfig,
    endpoint::JsonEndpoint,
    restart::JsonRestart,
    spec::JsonSpec,
    version::JsonApiVersion,
};
pub use user::{
    token::{JsonNewToken, JsonToken, JsonTokens, TokenUuid},
    JsonUpdateUser, JsonUser, JsonUsers, UserUuid,
};

pub const BENCHER_CONSOLE_PORT: u16 = 3000;
pub const LOCALHOST_BENCHER_URL_STR: &str = "http://localhost:3000";
pub const DEVEL_BENCHER_URL_STR: &str = "https://devel--bencher.netlify.app";
pub const PROD_BENCHER_URL_STR: &str = "https://bencher.dev";

#[cfg(debug_assertions)]
pub const BENCHER_URL_STR: &str = LOCALHOST_BENCHER_URL_STR;
#[cfg(not(debug_assertions))]
pub const BENCHER_URL_STR: &str = PROD_BENCHER_URL_STR;

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
#[allow(clippy::panic)]
pub static PROD_BENCHER_URL: Lazy<url::Url> = Lazy::new(|| {
    PROD_BENCHER_URL_STR
        .parse()
        .unwrap_or_else(|e| panic!("Failed to parse endpoint \"{PROD_BENCHER_URL_STR}\": {e}"))
});
#[cfg(feature = "plus")]
pub fn is_bencher_cloud(url: &url::Url) -> bool {
    *url == *BENCHER_URL || *url == *DEVEL_BENCHER_URL || *url == *PROD_BENCHER_URL
}

// Dynamic and/or Private Ports (49152-65535)
// https://www.iana.org/assignments/service-names-port-numbers/service-names-port-numbers.xhtml?search=61016
pub const BENCHER_API_PORT: u16 = 61016;
pub const LOCALHOST_BENCHER_API_URL_STR: &str = "http://localhost:61016";
pub const DEV_BENCHER_API_URL_STR: &str = "https://bencher-api-dev.fly.dev";
pub const TEST_BENCHER_API_URL_STR: &str = "https://bencher-api-test.fly.dev";
pub const PROD_BENCHER_API_URL_STR: &str = "https://api.bencher.dev";

#[cfg(debug_assertions)]
pub const BENCHER_API_URL_STR: &str = LOCALHOST_BENCHER_API_URL_STR;
#[cfg(not(debug_assertions))]
pub const BENCHER_API_URL_STR: &str = PROD_BENCHER_API_URL_STR;

#[allow(clippy::panic)]
pub static BENCHER_API_URL: Lazy<url::Url> = Lazy::new(|| {
    BENCHER_API_URL_STR
        .parse()
        .unwrap_or_else(|e| panic!("Failed to parse endpoint \"{BENCHER_API_URL_STR}\": {e}"))
});
#[allow(clippy::panic)]
pub static LOCALHOST_BENCHER_API_URL: Lazy<url::Url> = Lazy::new(|| {
    LOCALHOST_BENCHER_API_URL_STR.parse().unwrap_or_else(|e| {
        panic!("Failed to parse endpoint \"{LOCALHOST_BENCHER_API_URL_STR}\": {e}")
    })
});
#[allow(clippy::panic)]
pub static DEV_BENCHER_API_URL: Lazy<url::Url> = Lazy::new(|| {
    DEV_BENCHER_API_URL_STR
        .parse()
        .unwrap_or_else(|e| panic!("Failed to parse endpoint \"{DEV_BENCHER_API_URL_STR}\": {e}"))
});
#[allow(clippy::panic)]
pub static TEST_BENCHER_API_URL: Lazy<url::Url> = Lazy::new(|| {
    TEST_BENCHER_API_URL_STR
        .parse()
        .unwrap_or_else(|e| panic!("Failed to parse endpoint \"{TEST_BENCHER_API_URL_STR}\": {e}"))
});
#[allow(clippy::panic)]
pub static PROD_BENCHER_API_URL: Lazy<url::Url> = Lazy::new(|| {
    PROD_BENCHER_API_URL_STR
        .parse()
        .unwrap_or_else(|e| panic!("Failed to parse endpoint \"{PROD_BENCHER_API_URL_STR}\": {e}"))
});

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonAny {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUuids(pub Vec<JsonUuid>);

crate::from_vec!(JsonUuids[JsonUuid]);

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUuid {
    pub uuid: uuid::Uuid,
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
