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
    NameId,
    NonEmpty,
    ResourceId,
    ResourceName,
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
    ModelUuid,
    AlertUuid,
    UserUuid,
    TokenUuid
);

#[cfg(feature = "plus")]
from_client!(Entitlements, ExpirationMonth, ExpirationYear);

/// This type allows for forwards compatibility with the API response types.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct JsonValue(pub serde_json::Value);

impl From<JsonValue> for serde_json::Value {
    fn from(json: JsonValue) -> Self {
        json.0
    }
}

impl From<()> for JsonValue {
    fn from(json: ()) -> Self {
        Self(serde_json::json!(json))
    }
}

// This is a bit of a kludge, but it should always work!
macro_rules! try_from_client {
    ($($name:ident),*) => {
        $(
            impl From<types::$name> for JsonValue  {
                fn from(json: types::$name) -> Self {
                    Self(serde_json::json!(json))
                }
            }

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
    JsonModel,
    JsonTestbeds,
    JsonTestbed,
    JsonThresholds,
    JsonThreshold,
    JsonSignup,
    JsonLogin,
    JsonConfirm,
    JsonAccept,
    JsonAuthAck,
    JsonAuthUser,
    JsonBackupCreated,
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
try_from_client!(JsonOAuth, JsonPlan, JsonUsage, JsonServerStats);

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

macro_rules! into_uuids {
    ($($list:ident[$name:ident]),*) => {
        $(
            impl TryFrom<types::$list> for bencher_json::JsonUuids {
                type Error = serde_json::Error;

                fn try_from(list: types::$list) -> Result<Self, Self::Error> {
                    Ok(Self(list.0.into_iter().map(
                        |json|  {
                            let types::$name { uuid, .. } = json;
                            bencher_json::JsonUuid {
                                uuid: uuid.into(),
                            }
                        }).collect()
                    ))
                }
            }
        )*
    };
}

into_uuids!(
    JsonMembers[JsonMember],
    JsonOrganizations[JsonOrganization],
    JsonAlerts[JsonAlert],
    JsonBenchmarks[JsonBenchmark],
    JsonBranches[JsonBranch],
    JsonMeasures[JsonMeasure],
    JsonProjects[JsonProject],
    JsonReports[JsonReport],
    JsonTestbeds[JsonTestbed],
    JsonThresholds[JsonThreshold]
);

macro_rules! into_uuid {
    ($($name:ident),*) => {
        $(
            impl TryFrom<types::$name> for bencher_json::JsonUuid {
                type Error = serde_json::Error;

                fn try_from(json: types::$name) -> Result<Self, Self::Error> {
                    let types::$name { uuid, .. } = json;
                    Ok(bencher_json::JsonUuid {
                        uuid: uuid.into(),
                    })
                }
            }
        )*
    };
}

into_uuid!(
    JsonMember,
    JsonOrganization,
    JsonAlert,
    JsonBenchmark,
    JsonBranch,
    JsonMeasure,
    JsonProject,
    JsonReport,
    JsonModel,
    JsonTestbed,
    JsonThreshold
);
