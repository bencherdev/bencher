mod codegen {
    #![allow(clippy::all)]
    include!(concat!(env!("OUT_DIR"), "/codegen.rs"));
}

pub use codegen::*;

macro_rules! from_client {
    ($($name:ident),*) => {
        $(
            impl From<bencher_json::$name> for types::$name  {
                fn from(json: bencher_json::$name) -> Self {
                    Self(json.into())
                }
            }
        )*
    };
}

from_client!(
    BenchmarkName,
    Boundary,
    BranchName,
    Email,
    GitHash,
    Jwt,
    NonEmpty,
    ResourceId,
    Slug,
    Url,
    UserName
);

#[cfg(feature = "plus")]
from_client!(CardCvc, CardNumber, ExpirationMonth, ExpirationYear);

// This is a bit of a kludge, but it should always work!
macro_rules! try_from_client {
    ($($name:ident),*) => {
        $(
            impl std::convert::TryFrom<types::$name> for bencher_json::$name  {
                type Error = serde_json::Error;

                fn try_from(json: types::$name) -> Result<Self, Self::Error> {
                    serde_json::from_value::<Self>(serde_json::json!(json))
                }
            }
        )*
    };
}

try_from_client!(
    JsonEmpty,
    JsonMember,
    JsonMembers,
    JsonAllowed,
    JsonOrganization,
    JsonOrganizations,
    JsonAlerts,
    JsonAlert,
    JsonBenchmarks,
    JsonBenchmark,
    JsonBranches,
    JsonBranch,
    JsonMetricKinds,
    JsonMetricKind,
    JsonProjects,
    JsonProject,
    JsonPerf,
    JsonReports,
    JsonReport,
    JsonStatistic,
    JsonTestbeds,
    JsonTestbed,
    JsonThresholds,
    JsonThreshold,
    JsonConfirm,
    JsonLogin,
    JsonSignup,
    JsonConfig,
    JsonEndpoint,
    JsonApiVersion,
    JsonPing,
    JsonTokens,
    JsonToken,
    JsonUser
);

#[cfg(feature = "plus")]
try_from_client!(JsonPlan, JsonEntitlements);
