use std::fmt;

use bencher_valid::{DateTime, ResourceName, Window};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{BenchmarkUuid, BranchUuid, MeasureUuid, ProjectUuid, TestbedUuid};

crate::typed_uuid::typed_uuid!(PlotUuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[allow(clippy::struct_excessive_bools)]
pub struct JsonNewPlot {
    /// The name of the testbed.
    /// Maximum length is 64 characters.
    pub name: ResourceName,
    /// Display metric lower values.
    pub lower_value: bool,
    /// Display metric upper values.
    pub upper_value: bool,
    /// Display lower boundary limits.
    pub lower_boundary: bool,
    /// Display upper boundary limits.
    pub upper_boundary: bool,
    /// The x-axis to use for the plot.
    pub x_axis: XAxis,
    /// The window of time for the plot, in seconds.
    /// Metrics outside of this window will be omitted.
    pub window: Window,
    /// The branches to include in the plot.
    /// At least one branch must be specified.
    pub branches: Vec<BranchUuid>,
    /// The testbeds to include in the plot.
    /// At least one testbed must be specified.
    pub testbeds: Vec<TestbedUuid>,
    /// The benchmarks to include in the plot.
    /// At least one benchmark must be specified.
    pub benchmarks: Vec<BenchmarkUuid>,
    /// The measures to include in the plot.
    /// At least one measure must be specified.
    pub measures: Vec<MeasureUuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPlots(pub Vec<JsonPlot>);

crate::from_vec!(JsonPlots[JsonPlot]);

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[allow(clippy::struct_excessive_bools)]
pub struct JsonPlot {
    pub uuid: PlotUuid,
    pub project: ProjectUuid,
    pub name: ResourceName,
    pub lower_value: bool,
    pub upper_value: bool,
    pub lower_boundary: bool,
    pub upper_boundary: bool,
    pub x_axis: XAxis,
    pub window: Window,
    pub branches: Vec<BranchUuid>,
    pub testbeds: Vec<TestbedUuid>,
    pub benchmarks: Vec<BenchmarkUuid>,
    pub measures: Vec<MeasureUuid>,
    pub created: DateTime,
    pub modified: DateTime,
}

impl fmt::Display for JsonPlot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUpdatePlot {
    /// The new name of the testbed.
    /// Maximum length is 64 characters.
    pub name: Option<ResourceName>,
}

const DATE_TIME_INT: i32 = 0;
const BRANCH_VERSION_INT: i32 = 1;

#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, Default, derive_more::Display, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Integer))]
#[serde(rename_all = "snake_case")]
#[repr(i32)]
pub enum XAxis {
    #[default]
    DateTime = DATE_TIME_INT,
    BranchVersion = BRANCH_VERSION_INT,
}

#[cfg(feature = "db")]
mod plot_axis {
    use super::{XAxis, BRANCH_VERSION_INT, DATE_TIME_INT};

    #[derive(Debug, thiserror::Error)]
    pub enum XAxisError {
        #[error("Invalid plot axis value: {0}")]
        Invalid(i32),
    }

    impl<DB> diesel::serialize::ToSql<diesel::sql_types::Integer, DB> for XAxis
    where
        DB: diesel::backend::Backend,
        i32: diesel::serialize::ToSql<diesel::sql_types::Integer, DB>,
    {
        fn to_sql<'b>(
            &'b self,
            out: &mut diesel::serialize::Output<'b, '_, DB>,
        ) -> diesel::serialize::Result {
            match self {
                Self::DateTime => DATE_TIME_INT.to_sql(out),
                Self::BranchVersion => BRANCH_VERSION_INT.to_sql(out),
            }
        }
    }

    impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Integer, DB> for XAxis
    where
        DB: diesel::backend::Backend,
        i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, DB>,
    {
        fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            match i32::from_sql(bytes)? {
                DATE_TIME_INT => Ok(Self::DateTime),
                BRANCH_VERSION_INT => Ok(Self::BranchVersion),
                value => Err(Box::new(XAxisError::Invalid(value))),
            }
        }
    }
}
