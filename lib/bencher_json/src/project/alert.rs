use ordered_float::OrderedFloat;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    BranchUuid, DateTime, DateTimeMillis, JsonBenchmark, JsonBoundary, JsonMetricTriple,
    JsonThreshold, JsonVariant, MeasureUuid, TestbedUuid, ThresholdUuid,
};

use super::{boundary::BoundaryLimit, report::Iteration, report::ReportUuid};

crate::typed_uuid::typed_uuid!(AlertUuid);

/// The most entries any list in a [`JsonUpdateAlerts`] may hold.
pub const MAX_UPDATE_ALERTS: usize = 255;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonAlerts(pub Vec<JsonAlert>);

crate::from_vec!(JsonAlerts[JsonAlert]);

#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonAlert {
    pub uuid: AlertUuid,
    pub report: ReportUuid,
    pub iteration: Iteration,
    pub benchmark: JsonBenchmark,
    /// The variant the alert fired on.
    ///
    /// Two variants of one benchmark raise two alerts, and this is what tells
    /// them apart.
    pub variant: JsonVariant,
    /// The value the alert fired on.
    ///
    /// The name that value was reported under is the threshold's, at
    /// `threshold.metric`, because that is the name the threshold checks.
    pub value: OrderedFloat<f64>,
    /// Deprecated. The metric triple, present only when the checked row is a
    /// `value` row.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metric: Option<JsonMetricTriple>,
    pub threshold: JsonThreshold,
    pub boundary: JsonBoundary,
    pub limit: BoundaryLimit,
    pub status: AlertStatus,
    pub created: DateTime,
    pub modified: DateTime,
}

const ACTIVE_INT: i32 = 0;
const DISMISSED_INT: i32 = 1;
const SILENCED_INT: i32 = 10;

#[typeshare::typeshare]
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, derive_more::Display, Serialize, Deserialize,
)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Integer))]
#[serde(rename_all = "snake_case")]
#[repr(i32)]
pub enum AlertStatus {
    #[default]
    /// The alert is active.
    Active = ACTIVE_INT,
    /// The alert has been dismissed by a user.
    Dismissed = DISMISSED_INT,
    /// The alert has been silenced by the system.
    Silenced = SILENCED_INT,
}

impl From<UpdateAlertStatus> for AlertStatus {
    fn from(status: UpdateAlertStatus) -> Self {
        match status {
            UpdateAlertStatus::Active => Self::Active,
            UpdateAlertStatus::Dismissed => Self::Dismissed,
        }
    }
}

#[cfg(feature = "db")]
mod alert_status {
    use super::{ACTIVE_INT, AlertStatus, DISMISSED_INT, SILENCED_INT};

    #[derive(Debug, thiserror::Error)]
    pub enum AlertStatusError {
        #[error("Invalid alert status value: {0}")]
        Invalid(i32),
    }

    impl<DB> diesel::serialize::ToSql<diesel::sql_types::Integer, DB> for AlertStatus
    where
        DB: diesel::backend::Backend,
        i32: diesel::serialize::ToSql<diesel::sql_types::Integer, DB>,
    {
        fn to_sql<'b>(
            &'b self,
            out: &mut diesel::serialize::Output<'b, '_, DB>,
        ) -> diesel::serialize::Result {
            match self {
                Self::Active => ACTIVE_INT.to_sql(out),
                Self::Dismissed => DISMISSED_INT.to_sql(out),
                Self::Silenced => SILENCED_INT.to_sql(out),
            }
        }
    }

    impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Integer, DB> for AlertStatus
    where
        DB: diesel::backend::Backend,
        i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, DB>,
    {
        fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            match i32::from_sql(bytes)? {
                ACTIVE_INT => Ok(Self::Active),
                DISMISSED_INT => Ok(Self::Dismissed),
                SILENCED_INT => Ok(Self::Silenced),
                value => Err(Box::new(AlertStatusError::Invalid(value))),
            }
        }
    }
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUpdateAlert {
    /// The new status of the alert.
    pub status: Option<UpdateAlertStatus>,
}

impl JsonUpdateAlert {
    pub fn is_status_only(&self) -> bool {
        let Self { status: _ } = self;
        true
    }
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum UpdateAlertStatus {
    /// The alert is active.
    Active,
    /// The alert has been dismissed by a user.
    Dismissed,
}

/// A status change for many alerts at once, selected by either `alerts` or `filter`.
///
/// Silenced alerts never change, and an alert already in the new status is left as it is.
#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUpdateAlerts {
    /// The new status of the alerts.
    pub status: UpdateAlertStatus,
    /// The alerts to change, at most 255.
    #[serde(default)]
    pub alerts: Option<Vec<AlertUuid>>,
    /// Change every alert that matches.
    #[serde(default)]
    pub filter: Option<JsonAlertsFilter>,
}

/// The alerts that match every given field.
///
/// An empty or absent list matches every value, and each list holds at most 255 entries.
#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonAlertsFilter {
    /// The current status of the alerts.
    #[serde(default)]
    pub status: Option<AlertStatus>,
    /// The branches the alerts were raised on.
    #[serde(default)]
    pub branches: Vec<BranchUuid>,
    /// The testbeds the alerts were raised on.
    #[serde(default)]
    pub testbeds: Vec<TestbedUuid>,
    /// The measures the alerts were raised on.
    #[serde(default)]
    pub measures: Vec<MeasureUuid>,
    /// The thresholds that raised the alerts.
    #[serde(default)]
    pub thresholds: Vec<ThresholdUuid>,
    /// The earliest time an alert was created, in milliseconds, inclusive.
    #[serde(default)]
    pub start_time: Option<DateTimeMillis>,
    /// The latest time an alert was created, in milliseconds, inclusive.
    #[serde(default)]
    pub end_time: Option<DateTimeMillis>,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUpdatedAlerts {
    /// The number of alerts whose status changed.
    pub changed: u32,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPerfAlert {
    pub uuid: AlertUuid,
    pub limit: BoundaryLimit,
    pub status: AlertStatus,
    pub modified: DateTime,
}
