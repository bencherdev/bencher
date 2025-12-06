#![cfg(feature = "plus")]

use bencher_valid::{DateTime, ResourceName};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{JsonOrganizations, JsonUsers, ProjectUuid};

crate::typed_uuid::typed_uuid!(ServerUuid);

/// A Bencher server instance
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonServer {
    /// The server UUID
    pub uuid: ServerUuid,
    /// The date the server was created
    pub created: DateTime,
    /// The current version of the server
    pub version: Option<String>,
}

/// Bencher server stats
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonServerStats {
    /// The Bencher server instance
    pub server: JsonServer,
    /// Timestamp of the stats
    pub timestamp: DateTime,
    /// Server organizations
    pub organizations: Option<JsonOrganizations>,
    /// Server admins
    pub admins: Option<JsonUsers>,
    /// Number of users (created)
    pub users: Option<JsonCohort>,
    /// Number of projects (created)
    pub projects: Option<JsonCohort>,
    /// Number of unclaimed projects (created)
    pub projects_unclaimed: Option<JsonCohort>,
    /// Number of claimed projects (created)
    pub projects_claimed: Option<JsonCohort>,
    /// Number of active projects (with at least one report)
    pub active_projects: Option<JsonCohort>,
    /// Number of active unclaimed projects (with at least one report)
    pub active_projects_unclaimed: Option<JsonCohort>,
    /// Number of active claimed projects (with at least one report)
    pub active_projects_claimed: Option<JsonCohort>,
    /// Number of reports (created)
    pub reports: Option<JsonCohort>,
    /// Number of unclaimed reports (created)
    pub reports_unclaimed: Option<JsonCohort>,
    /// Number of claimed reports (created)
    pub reports_claimed: Option<JsonCohort>,
    /// Number of reports per active project (created)
    pub reports_per_project: Option<JsonCohortAvg>,
    /// Number of reports per active unclaimed project (created)
    pub reports_per_project_unclaimed: Option<JsonCohortAvg>,
    /// Number of reports per active claimed project (created)
    pub reports_per_project_claimed: Option<JsonCohortAvg>,
    /// Number of metrics (created)
    pub metrics: Option<JsonCohort>,
    /// Number of unclaimed metrics (created)
    pub metrics_unclaimed: Option<JsonCohort>,
    /// Number of claimed metrics (created)
    pub metrics_claimed: Option<JsonCohort>,
    /// Number of metrics per report (created)
    pub metrics_per_report: Option<JsonCohortAvg>,
    /// Number of metrics per unclaimed report (created)
    pub metrics_per_report_unclaimed: Option<JsonCohortAvg>,
    /// Number of metrics per claimed report (created)
    pub metrics_per_report_claimed: Option<JsonCohortAvg>,
    /// Top 10 projects
    pub top_projects: Option<JsonTopCohort>,
    /// Top 10 unclaimed projects
    pub top_projects_unclaimed: Option<JsonTopCohort>,
    /// Top 10 claimed projects
    pub top_projects_claimed: Option<JsonTopCohort>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonCohort {
    pub week: u64,
    pub month: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonCohortAvg {
    pub week: f64,
    pub month: f64,
    pub total: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonTopCohort {
    pub week: JsonTopProjects,
    pub month: JsonTopProjects,
    pub total: JsonTopProjects,
}

pub type JsonTopProjects = Vec<JsonTopProject>;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonTopProject {
    pub name: ResourceName,
    pub uuid: ProjectUuid,
    pub metrics: u64,
    pub percentage: f64,
}

// Marker structs for self-hosted server telemetry boolean query parameters

#[derive(Debug, Clone, Copy)]
pub enum BooleanParam<T> {
    True(T),
    False,
}

impl<T> From<BooleanParam<T>> for bool {
    fn from(param: BooleanParam<T>) -> Self {
        matches!(param, BooleanParam::True(_))
    }
}

impl<T> Serialize for BooleanParam<T>
where
    T: ParamKey,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            BooleanParam::True(_) => [(T::KEY, "true")].serialize(serializer),
            BooleanParam::False => serializer.serialize_none(),
        }
    }
}

impl<'de, T> Deserialize<'de> for BooleanParam<T>
where
    T: Default,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let opt = Option::<bool>::deserialize(deserializer)?;
        match opt {
            Some(true) => Ok(BooleanParam::True(T::default())),
            Some(false) | None => Ok(BooleanParam::False),
        }
    }
}

#[cfg(feature = "schema")]
impl<T> JsonSchema for BooleanParam<T> {
    fn schema_name() -> String {
        "BooleanParam".to_owned()
    }

    fn json_schema(_gen: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
        schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::Boolean.into()),
            metadata: Some(Box::new(schemars::schema::Metadata {
                description: Some("Optional boolean parameter".to_owned()),
                ..Default::default()
            })),
            ..Default::default()
        }
        .into()
    }
}

trait ParamKey {
    const KEY: &'static str;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SelfHostedStartup;

impl ParamKey for SelfHostedStartup {
    const KEY: &'static str = "startup";
}

#[cfg(test)]
mod tests {
    use super::{BooleanParam, SelfHostedStartup};

    #[test]
    fn test_boolean_param_from_true() {
        let param = BooleanParam::True(SelfHostedStartup);
        assert!(bool::from(param));
    }

    #[test]
    fn test_boolean_param_from_false() {
        let param: BooleanParam<SelfHostedStartup> = BooleanParam::False;
        assert!(!bool::from(param));
    }

    #[test]
    fn test_boolean_param_deserialize_null() {
        let json = "null";
        let param: BooleanParam<SelfHostedStartup> = serde_json::from_str(json).unwrap();
        assert!(matches!(param, BooleanParam::False));
    }

    #[test]
    fn test_boolean_param_deserialize_true() {
        let json = "true";
        let param: BooleanParam<SelfHostedStartup> = serde_json::from_str(json).unwrap();
        assert!(matches!(param, BooleanParam::True(SelfHostedStartup)));
    }

    #[test]
    fn test_boolean_param_deserialize_false() {
        let json = "false";
        let param: BooleanParam<SelfHostedStartup> = serde_json::from_str(json).unwrap();
        assert!(matches!(param, BooleanParam::False));
    }

    #[test]
    fn test_boolean_param_serialize_true() {
        let param = BooleanParam::True(SelfHostedStartup);
        let json = serde_json::to_string(&param).unwrap();
        assert_eq!(json, r#"[["startup","true"]]"#);
    }

    #[test]
    fn test_boolean_param_serialize_false() {
        let param: BooleanParam<SelfHostedStartup> = BooleanParam::False;
        let json = serde_json::to_string(&param).unwrap();
        assert_eq!(json, "null");
    }
}
