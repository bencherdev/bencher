mod codegen {
    #![allow(
        unused_qualifications,
        variant_size_differences,
        clippy::all,
        clippy::cargo,
        clippy::pedantic,
        clippy::restriction
    )]
    include!(concat!(env!("OUT_DIR"), "/codegen.rs"));
}
mod client;

pub use bencher_json as json;
pub use client::{BencherClient, BencherClientBuilder, ClientError};
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
    SampleSize,
    Slug,
    Url,
    UserName,
    Window
);

from_client!(
    OrganizationUuid,
    ProjectUuid,
    ReportUuid,
    BranchUuid,
    TestbedUuid,
    BenchmarkUuid,
    MeasureUuid,
    ThresholdUuid,
    StatisticUuid,
    AlertUuid,
    UserUuid,
    TokenUuid
);

#[cfg(feature = "plus")]
from_client!(
    CardCvc,
    CardNumber,
    Entitlements,
    ExpirationMonth,
    ExpirationYear
);

// This is required because by default `()` deserializes with `Infallible` as its error type.
// That makes it a lot more complicated to implement the client `send_with` method.
// So this just shims in `serde_json::Error` as the error type to remove having to multiplex over the two.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct JsonUnit;

impl TryFrom<()> for JsonUnit {
    type Error = serde_json::Error;

    fn try_from(_json: ()) -> Result<Self, Self::Error> {
        Ok(Self)
    }
}

// This is a bit of a kludge, but it should always work!
macro_rules! try_from_client {
    ($($name:ident),*) => {
        $(
            impl TryFrom<types::$name> for bencher_json::$name  {
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
    JsonCreated,
    JsonMember,
    JsonMembers,
    JsonAllowed,
    JsonOrganization,
    JsonOrganizations,
    JsonAlerts,
    JsonAlertStats,
    JsonAlert,
    JsonBenchmarks,
    JsonBenchmark,
    JsonBranches,
    JsonBranch,
    JsonMeasures,
    JsonMeasure,
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
    JsonAuth,
    JsonAuthUser,
    JsonLogin,
    JsonSignup,
    JsonConfig,
    JsonEndpoint,
    JsonApiVersion,
    JsonSpec,
    JsonPing,
    JsonTokens,
    JsonToken,
    JsonUser
);

#[cfg(feature = "plus")]
try_from_client!(JsonPlan, JsonUsage, JsonServerStats);

impl From<bencher_json::DateTime> for types::DateTime {
    fn from(date_time: bencher_json::DateTime) -> Self {
        Self(date_time.into_inner())
    }
}

impl From<bencher_json::DateTimeMillis> for types::DateTimeMillis {
    fn from(date_time: bencher_json::DateTimeMillis) -> Self {
        Self(types::TimestampMillis(date_time.into()))
    }
}

macro_rules! into_created {
    ($($name:ident),*) => {
        $(
            impl From<types::$name> for bencher_json::JsonCreated {
                fn from(json: types::$name) -> Self {
                    let types::$name { created, .. } = json;
                    bencher_json::JsonCreated {
                        created: created.0.into(),
                    }
                }
            }
        )*
    };
}

into_created!(
    JsonMember,
    JsonOrganization,
    JsonAlert,
    JsonBenchmark,
    JsonBranch,
    JsonMeasure,
    JsonProject,
    JsonReport,
    JsonStatistic,
    JsonTestbed,
    JsonThreshold
);
