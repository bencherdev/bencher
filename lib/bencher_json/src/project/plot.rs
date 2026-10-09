use std::fmt;

use bencher_valid::{DateTime, Index, MetricName, ResourceName, Window};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{self, Visitor},
};

use crate::{BenchmarkUuid, BranchUuid, MeasureUuid, ParameterFilter, ProjectUuid, TestbedUuid};

use super::perf::MAX_DIMENSION_ENTRIES;

crate::typed_uuid::typed_uuid!(PlotUuid);

/// The most plots a project keeps.
pub const MAX_PLOTS: u8 = 64;

#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[expect(
    clippy::struct_excessive_bools,
    reason = "plot display flags are independent toggles"
)]
pub struct JsonNewPlot {
    /// The index of the plot.
    /// Maximum index is 64.
    pub index: Option<Index>,
    /// The title of the plot.
    /// Maximum length is 64 characters.
    pub title: Option<ResourceName>,
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
    /// The y-axis scale to use for the plot.
    /// Defaults to `auto` when omitted.
    #[serde(default)]
    pub y_axis: YAxis,
    /// How the plot lays out two or more measures:
    /// `dual` draws them on one chart with a y-axis each, and `stacked` draws each on its own chart.
    /// If not set, the plot uses the default layout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<PlotLayout>,
    /// The window of time for the plot, in seconds.
    /// Metrics outside of this window will be omitted.
    pub window: Window,
    /// The branches to include in the plot.
    /// At least one branch must be specified, and at most 8.
    #[cfg_attr(feature = "schema", schemars(length(max = "MAX_DIMENSION_ENTRIES")))]
    pub branches: Vec<BranchUuid>,
    /// The testbeds to include in the plot.
    /// At least one testbed must be specified, and at most 8.
    #[cfg_attr(feature = "schema", schemars(length(max = "MAX_DIMENSION_ENTRIES")))]
    pub testbeds: Vec<TestbedUuid>,
    /// The benchmarks to include in the plot.
    /// At least one benchmark must be specified, and at most 8.
    #[cfg_attr(feature = "schema", schemars(length(max = "MAX_DIMENSION_ENTRIES")))]
    pub benchmarks: Vec<BenchmarkUuid>,
    /// The variants to include in the plot, as a parameters filter.
    /// A variant matches when any entry in the filter is a subset of its parameters.
    /// If not set, or set to an empty list, the plot includes every variant.
    /// At most 8 entries may be specified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<ParameterFilter>,
    /// The measures to include in the plot.
    /// At least one measure must be specified, and at most 8.
    #[cfg_attr(feature = "schema", schemars(length(max = "MAX_DIMENSION_ENTRIES")))]
    pub measures: Vec<MeasureUuid>,
    /// The metrics to draw, by name.
    /// If not set, or set to an empty list, the plot draws every metric.
    /// At most 8 names may be specified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metrics: Option<MetricFilter>,
    /// The keys of the lines to hide.
    /// If not set, or set to an empty list, the plot hides no line.
    /// At most 64 keys may be specified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hidden: Option<LineKeys>,
    /// The key of the line to focus.
    /// If not set, the plot focuses no line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus: Option<LineKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPlots(pub Vec<JsonPlot>);

crate::from_vec!(JsonPlots[JsonPlot]);

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[expect(
    clippy::struct_excessive_bools,
    reason = "plot display flags are independent toggles"
)]
pub struct JsonPlot {
    pub uuid: PlotUuid,
    pub project: ProjectUuid,
    pub title: Option<ResourceName>,
    pub lower_value: bool,
    pub upper_value: bool,
    pub lower_boundary: bool,
    pub upper_boundary: bool,
    pub x_axis: XAxis,
    pub y_axis: YAxis,
    /// The layout of the plot's measures.
    /// Absent when the plot uses the default layout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<PlotLayout>,
    pub window: Window,
    pub branches: Vec<BranchUuid>,
    pub testbeds: Vec<TestbedUuid>,
    pub benchmarks: Vec<BenchmarkUuid>,
    /// The variants this plot draws, in canonical order.
    /// Absent when the plot draws every variant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<ParameterFilter>,
    pub measures: Vec<MeasureUuid>,
    /// The metrics this plot draws, by name.
    /// Absent when the plot draws every metric.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metrics: Option<MetricFilter>,
    /// The keys of the lines this plot hides.
    /// Absent when the plot hides no line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hidden: Option<LineKeys>,
    /// The key of the line this plot focuses.
    /// Absent when the plot focuses no line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus: Option<LineKey>,
    pub created: DateTime,
    pub modified: DateTime,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(untagged)]
pub enum JsonUpdatePlot {
    Patch(JsonPlotPatch),
    Null(JsonPlotPatchNull),
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPlotPatch {
    /// The new index for the plot.
    /// Maximum index is 64.
    pub index: Option<Index>,
    /// The new title of the plot.
    /// Set to `null` to remove the current title.
    /// Maximum length is 64 characters.
    pub title: Option<ResourceName>,
    /// Display metric lower values.
    pub lower_value: Option<bool>,
    /// Display metric upper values.
    pub upper_value: Option<bool>,
    /// Display lower boundary limits.
    pub lower_boundary: Option<bool>,
    /// Display upper boundary limits.
    pub upper_boundary: Option<bool>,
    /// The x-axis to use for the plot.
    pub x_axis: Option<XAxis>,
    /// The y-axis scale to use for the plot.
    pub y_axis: Option<YAxis>,
    /// How the plot lays out two or more measures.
    /// Set to `null` to use the default layout.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<PlotLayout>"))]
    pub layout: Option<Option<PlotLayout>>,
    /// The window of time for the plot, in seconds.
    /// Metrics outside of this window will be omitted.
    pub window: Option<Window>,
    /// The branches to include in the plot.
    /// Replaces the current branches for the plot.
    /// At least one branch must be specified, and at most 8.
    #[cfg_attr(feature = "schema", schemars(length(max = "MAX_DIMENSION_ENTRIES")))]
    pub branches: Option<Vec<BranchUuid>>,
    /// The testbeds to include in the plot.
    /// Replaces the current testbeds for the plot.
    /// At least one testbed must be specified, and at most 8.
    #[cfg_attr(feature = "schema", schemars(length(max = "MAX_DIMENSION_ENTRIES")))]
    pub testbeds: Option<Vec<TestbedUuid>>,
    /// The benchmarks to include in the plot.
    /// Replaces the current benchmarks for the plot.
    /// At least one benchmark must be specified, and at most 8.
    #[cfg_attr(feature = "schema", schemars(length(max = "MAX_DIMENSION_ENTRIES")))]
    pub benchmarks: Option<Vec<BenchmarkUuid>>,
    /// The variants to include in the plot, as a parameters filter.
    /// Replaces the current filter for the plot.
    /// Set to `null` or to an empty list to include every variant again.
    /// At most 8 entries may be specified.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<ParameterFilter>,
    /// The measures to include in the plot.
    /// Replaces the current measures for the plot.
    /// At least one measure must be specified, and at most 8.
    #[cfg_attr(feature = "schema", schemars(length(max = "MAX_DIMENSION_ENTRIES")))]
    pub measures: Option<Vec<MeasureUuid>>,
    /// The metrics to draw, by name.
    /// Replaces the current metrics for the plot.
    /// Set to `null` or to an empty list to draw every metric again.
    /// At most 8 names may be specified.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<MetricFilter>,
    /// The keys of the lines to hide.
    /// Replaces the current hidden lines for the plot.
    /// Set to `null` or to an empty list to hide no line.
    /// At most 64 keys may be specified.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hidden: Option<LineKeys>,
    /// The key of the line to focus.
    /// Set to `null` to focus no line.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<LineKey>"))]
    pub focus: Option<Option<LineKey>>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPlotPatchNull {
    pub index: Option<Index>,
    pub title: (),
    pub lower_value: Option<bool>,
    pub upper_value: Option<bool>,
    pub lower_boundary: Option<bool>,
    pub upper_boundary: Option<bool>,
    pub x_axis: Option<XAxis>,
    pub y_axis: Option<YAxis>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<PlotLayout>"))]
    pub layout: Option<Option<PlotLayout>>,
    pub window: Option<Window>,
    #[cfg_attr(feature = "schema", schemars(length(max = "MAX_DIMENSION_ENTRIES")))]
    pub branches: Option<Vec<BranchUuid>>,
    #[cfg_attr(feature = "schema", schemars(length(max = "MAX_DIMENSION_ENTRIES")))]
    pub testbeds: Option<Vec<TestbedUuid>>,
    #[cfg_attr(feature = "schema", schemars(length(max = "MAX_DIMENSION_ENTRIES")))]
    pub benchmarks: Option<Vec<BenchmarkUuid>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<ParameterFilter>,
    #[cfg_attr(feature = "schema", schemars(length(max = "MAX_DIMENSION_ENTRIES")))]
    pub measures: Option<Vec<MeasureUuid>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<MetricFilter>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hidden: Option<LineKeys>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<LineKey>"))]
    pub focus: Option<Option<LineKey>>,
}

impl<'de> Deserialize<'de> for JsonUpdatePlot {
    #[expect(clippy::too_many_lines, reason = "one match arm per updatable field")]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        const INDEX_FIELD: &str = "index";
        const TITLE_FIELD: &str = "title";
        const LOWER_VALUE_FIELD: &str = "lower_value";
        const UPPER_VALUE_FIELD: &str = "upper_value";
        const LOWER_BOUNDARY_FIELD: &str = "lower_boundary";
        const UPPER_BOUNDARY_FIELD: &str = "upper_boundary";
        const X_AXIS_FIELD: &str = "x_axis";
        const Y_AXIS_FIELD: &str = "y_axis";
        const LAYOUT_FIELD: &str = "layout";
        const WINDOW_FIELD: &str = "window";
        const BRANCHES_FIELD: &str = "branches";
        const TESTBEDS_FIELD: &str = "testbeds";
        const BENCHMARKS_FIELD: &str = "benchmarks";
        const PARAMETERS_FIELD: &str = "parameters";
        const MEASURES_FIELD: &str = "measures";
        const METRICS_FIELD: &str = "metrics";
        const HIDDEN_FIELD: &str = "hidden";
        const FOCUS_FIELD: &str = "focus";
        const FIELDS: &[&str] = &[
            INDEX_FIELD,
            TITLE_FIELD,
            LOWER_VALUE_FIELD,
            UPPER_VALUE_FIELD,
            LOWER_BOUNDARY_FIELD,
            UPPER_BOUNDARY_FIELD,
            X_AXIS_FIELD,
            Y_AXIS_FIELD,
            LAYOUT_FIELD,
            WINDOW_FIELD,
            BRANCHES_FIELD,
            TESTBEDS_FIELD,
            BENCHMARKS_FIELD,
            PARAMETERS_FIELD,
            MEASURES_FIELD,
            METRICS_FIELD,
            HIDDEN_FIELD,
            FOCUS_FIELD,
        ];

        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field {
            Index,
            Title,
            LowerValue,
            UpperValue,
            LowerBoundary,
            UpperBoundary,
            XAxis,
            YAxis,
            Layout,
            Window,
            Branches,
            Testbeds,
            Benchmarks,
            Parameters,
            Measures,
            Metrics,
            Hidden,
            Focus,
        }

        struct UpdatePlotVisitor;

        impl<'de> Visitor<'de> for UpdatePlotVisitor {
            type Value = JsonUpdatePlot;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("JsonUpdatePlot")
            }

            #[expect(clippy::too_many_lines, reason = "one match arm per updatable field")]
            fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
            where
                V: de::MapAccess<'de>,
            {
                let mut index = None;
                let mut title = None;
                let mut lower_value = None;
                let mut upper_value = None;
                let mut lower_boundary = None;
                let mut upper_boundary = None;
                let mut x_axis = None;
                let mut y_axis = None;
                let mut layout = None;
                let mut window = None;
                let mut branches = None;
                let mut testbeds = None;
                let mut benchmarks = None;
                let mut parameters: Option<Option<ParameterFilter>> = None;
                let mut measures = None;
                let mut metrics: Option<Option<MetricFilter>> = None;
                let mut hidden: Option<Option<LineKeys>> = None;
                let mut focus = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Index => {
                            if index.is_some() {
                                return Err(de::Error::duplicate_field(INDEX_FIELD));
                            }
                            index = Some(map.next_value()?);
                        },
                        Field::Title => {
                            if title.is_some() {
                                return Err(de::Error::duplicate_field(TITLE_FIELD));
                            }
                            title = Some(map.next_value()?);
                        },
                        Field::LowerValue => {
                            if lower_value.is_some() {
                                return Err(de::Error::duplicate_field(LOWER_VALUE_FIELD));
                            }
                            lower_value = Some(map.next_value()?);
                        },
                        Field::UpperValue => {
                            if upper_value.is_some() {
                                return Err(de::Error::duplicate_field(UPPER_VALUE_FIELD));
                            }
                            upper_value = Some(map.next_value()?);
                        },
                        Field::LowerBoundary => {
                            if lower_boundary.is_some() {
                                return Err(de::Error::duplicate_field(LOWER_BOUNDARY_FIELD));
                            }
                            lower_boundary = Some(map.next_value()?);
                        },
                        Field::UpperBoundary => {
                            if upper_boundary.is_some() {
                                return Err(de::Error::duplicate_field(UPPER_BOUNDARY_FIELD));
                            }
                            upper_boundary = Some(map.next_value()?);
                        },
                        Field::XAxis => {
                            if x_axis.is_some() {
                                return Err(de::Error::duplicate_field(X_AXIS_FIELD));
                            }
                            x_axis = Some(map.next_value()?);
                        },
                        Field::YAxis => {
                            if y_axis.is_some() {
                                return Err(de::Error::duplicate_field(Y_AXIS_FIELD));
                            }
                            y_axis = Some(map.next_value()?);
                        },
                        Field::Layout => {
                            if layout.is_some() {
                                return Err(de::Error::duplicate_field(LAYOUT_FIELD));
                            }
                            layout = Some(map.next_value()?);
                        },
                        Field::Window => {
                            if window.is_some() {
                                return Err(de::Error::duplicate_field(WINDOW_FIELD));
                            }
                            window = Some(map.next_value()?);
                        },
                        Field::Branches => {
                            if branches.is_some() {
                                return Err(de::Error::duplicate_field(BRANCHES_FIELD));
                            }
                            branches = Some(map.next_value()?);
                        },
                        Field::Testbeds => {
                            if testbeds.is_some() {
                                return Err(de::Error::duplicate_field(TESTBEDS_FIELD));
                            }
                            testbeds = Some(map.next_value()?);
                        },
                        Field::Benchmarks => {
                            if benchmarks.is_some() {
                                return Err(de::Error::duplicate_field(BENCHMARKS_FIELD));
                            }
                            benchmarks = Some(map.next_value()?);
                        },
                        Field::Parameters => {
                            if parameters.is_some() {
                                return Err(de::Error::duplicate_field(PARAMETERS_FIELD));
                            }
                            parameters = Some(map.next_value()?);
                        },
                        Field::Measures => {
                            if measures.is_some() {
                                return Err(de::Error::duplicate_field(MEASURES_FIELD));
                            }
                            measures = Some(map.next_value()?);
                        },
                        Field::Metrics => {
                            if metrics.is_some() {
                                return Err(de::Error::duplicate_field(METRICS_FIELD));
                            }
                            metrics = Some(map.next_value()?);
                        },
                        Field::Hidden => {
                            if hidden.is_some() {
                                return Err(de::Error::duplicate_field(HIDDEN_FIELD));
                            }
                            hidden = Some(map.next_value()?);
                        },
                        Field::Focus => {
                            if focus.is_some() {
                                return Err(de::Error::duplicate_field(FOCUS_FIELD));
                            }
                            focus = Some(map.next_value()?);
                        },
                    }
                }

                // An explicit null clears a list, as an empty list does.
                let parameters = parameters.map(Option::unwrap_or_default);
                let metrics = metrics.map(Option::unwrap_or_default);
                let hidden = hidden.map(Option::unwrap_or_default);

                Ok(match title {
                    Some(Some(title)) => Self::Value::Patch(JsonPlotPatch {
                        index,
                        title: Some(title),
                        lower_value,
                        upper_value,
                        lower_boundary,
                        upper_boundary,
                        x_axis,
                        y_axis,
                        layout,
                        window,
                        branches,
                        testbeds,
                        benchmarks,
                        parameters,
                        measures,
                        metrics,
                        hidden,
                        focus,
                    }),
                    Some(None) => Self::Value::Null(JsonPlotPatchNull {
                        index,
                        title: (),
                        lower_value,
                        upper_value,
                        lower_boundary,
                        upper_boundary,
                        x_axis,
                        y_axis,
                        layout,
                        window,
                        branches,
                        testbeds,
                        benchmarks,
                        parameters,
                        measures,
                        metrics,
                        hidden,
                        focus,
                    }),
                    None => Self::Value::Patch(JsonPlotPatch {
                        index,
                        title: None,
                        lower_value,
                        upper_value,
                        lower_boundary,
                        upper_boundary,
                        x_axis,
                        y_axis,
                        layout,
                        window,
                        branches,
                        testbeds,
                        benchmarks,
                        parameters,
                        measures,
                        metrics,
                        hidden,
                        focus,
                    }),
                })
            }
        }

        deserializer.deserialize_struct("JsonUpdatePlot", FIELDS, UpdatePlotVisitor)
    }
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum PlotKey {
    LowerValue,
    UpperValue,
    LowerBoundary,
    UpperBoundary,
    XAxis,
    YAxis,
}

pub const LOWER_VALUE: &str = "lower_value";
pub const UPPER_VALUE: &str = "upper_value";
pub const LOWER_BOUNDARY: &str = "lower_boundary";
pub const UPPER_BOUNDARY: &str = "upper_boundary";
pub const X_AXIS: &str = "x_axis";
pub const Y_AXIS: &str = "y_axis";

const DATE_TIME_INT: i32 = 0;
const VERSION_INT: i32 = 1;

#[typeshare::typeshare]
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, derive_more::Display, Serialize, Deserialize,
)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Integer))]
#[serde(rename_all = "snake_case")]
#[repr(i32)]
pub enum XAxis {
    #[default]
    DateTime = DATE_TIME_INT,
    Version = VERSION_INT,
}

#[cfg(feature = "db")]
mod plot_x_axis {
    use super::{DATE_TIME_INT, VERSION_INT, XAxis};

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
                Self::Version => VERSION_INT.to_sql(out),
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
                VERSION_INT => Ok(Self::Version),
                value => Err(Box::new(XAxisError::Invalid(value))),
            }
        }
    }
}

const AUTO_INT: i32 = 0;
const LINEAR_INT: i32 = 1;
const LOG_INT: i32 = 2;

#[typeshare::typeshare]
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, derive_more::Display, Serialize, Deserialize,
)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Integer))]
#[serde(rename_all = "snake_case")]
#[repr(i32)]
pub enum YAxis {
    /// Automatically adapt the y-axis scale to the data spread.
    #[default]
    Auto = AUTO_INT,
    /// A linear y-axis scale that shows true magnitudes.
    Linear = LINEAR_INT,
    /// A logarithmic y-axis scale.
    Log = LOG_INT,
}

#[cfg(feature = "db")]
mod plot_y_axis {
    use super::{AUTO_INT, LINEAR_INT, LOG_INT, YAxis};

    #[derive(Debug, thiserror::Error)]
    pub enum YAxisError {
        #[error("Invalid plot y-axis value: {0}")]
        Invalid(i32),
    }

    impl<DB> diesel::serialize::ToSql<diesel::sql_types::Integer, DB> for YAxis
    where
        DB: diesel::backend::Backend,
        i32: diesel::serialize::ToSql<diesel::sql_types::Integer, DB>,
    {
        fn to_sql<'b>(
            &'b self,
            out: &mut diesel::serialize::Output<'b, '_, DB>,
        ) -> diesel::serialize::Result {
            match self {
                Self::Auto => AUTO_INT.to_sql(out),
                Self::Linear => LINEAR_INT.to_sql(out),
                Self::Log => LOG_INT.to_sql(out),
            }
        }
    }

    impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Integer, DB> for YAxis
    where
        DB: diesel::backend::Backend,
        i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, DB>,
    {
        fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            match i32::from_sql(bytes)? {
                AUTO_INT => Ok(Self::Auto),
                LINEAR_INT => Ok(Self::Linear),
                LOG_INT => Ok(Self::Log),
                value => Err(Box::new(YAxisError::Invalid(value))),
            }
        }
    }
}

/// How a plot lays out two or more measures.
#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Integer))]
#[serde(rename_all = "snake_case")]
#[repr(i32)]
pub enum PlotLayout {
    /// One chart, with a y-axis for each measure.
    Dual = DUAL_INT,
    /// One chart for each measure.
    Stacked = STACKED_INT,
}

const DUAL_INT: i32 = 0;
const STACKED_INT: i32 = 1;

#[cfg(feature = "db")]
mod plot_layout {
    use super::{DUAL_INT, PlotLayout, STACKED_INT};

    #[derive(Debug, thiserror::Error)]
    pub enum PlotLayoutError {
        #[error("Invalid plot layout value: {0}")]
        Invalid(i32),
    }

    impl<DB> diesel::serialize::ToSql<diesel::sql_types::Integer, DB> for PlotLayout
    where
        DB: diesel::backend::Backend,
        i32: diesel::serialize::ToSql<diesel::sql_types::Integer, DB>,
    {
        fn to_sql<'b>(
            &'b self,
            out: &mut diesel::serialize::Output<'b, '_, DB>,
        ) -> diesel::serialize::Result {
            match self {
                Self::Dual => DUAL_INT.to_sql(out),
                Self::Stacked => STACKED_INT.to_sql(out),
            }
        }
    }

    impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Integer, DB> for PlotLayout
    where
        DB: diesel::backend::Backend,
        i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, DB>,
    {
        fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            match i32::from_sql(bytes)? {
                DUAL_INT => Ok(Self::Dual),
                STACKED_INT => Ok(Self::Stacked),
                value => Err(Box::new(PlotLayoutError::Invalid(value))),
            }
        }
    }
}

/// The most lines one plot may hide.
pub const MAX_HIDDEN_LINES: usize = 64;

/// The most characters in a line key.
pub const MAX_LINE_KEY_LEN: usize = 16;

/// The metrics a plot draws, by name, in the order they were first named.
///
/// The empty list draws every metric, so it is stored as `NULL`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
pub struct MetricFilter(Vec<MetricName>);

impl MetricFilter {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[must_use]
    pub fn names(&self) -> &[MetricName] {
        &self.0
    }
}

impl<'de> Deserialize<'de> for MetricFilter {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // The cap bounds what was written, before duplicates collapse.
        let names = Vec::<MetricName>::deserialize(deserializer)?;
        if names.len() > MAX_DIMENSION_ENTRIES {
            return Err(de::Error::custom(format!(
                "A plot may draw at most {MAX_DIMENSION_ENTRIES} metrics, found {}",
                names.len()
            )));
        }
        Ok(Self(first_occurrences(names)))
    }
}

/// The keys of the lines a plot hides, in the order they were first named.
///
/// The empty list hides no line, so it is stored as `NULL`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
pub struct LineKeys(Vec<LineKey>);

impl LineKeys {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[must_use]
    pub fn keys(&self) -> &[LineKey] {
        &self.0
    }
}

impl<'de> Deserialize<'de> for LineKeys {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // The cap bounds what was written, before duplicates collapse.
        let keys = Vec::<LineKey>::deserialize(deserializer)?;
        if keys.len() > MAX_HIDDEN_LINES {
            return Err(de::Error::custom(format!(
                "A plot may hide at most {MAX_HIDDEN_LINES} lines, found {}",
                keys.len()
            )));
        }
        Ok(Self(first_occurrences(keys)))
    }
}

fn first_occurrences<T: PartialEq>(items: Vec<T>) -> Vec<T> {
    let mut unique = Vec::with_capacity(items.len());
    for item in items {
        if !unique.contains(&item) {
            unique.push(item);
        }
    }
    unique
}

/// An opaque key naming one line of a plot.
/// A line key is 1 to 16 URL-safe characters: `A-Z`, `a-z`, `0-9`, `-`, and `_`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(try_from = "String")]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
pub struct LineKey(String);

#[derive(Debug, thiserror::Error)]
#[error(
    "Invalid line key {0:?}: a line key is 1 to {max} characters of A-Z, a-z, 0-9, - and _",
    max = MAX_LINE_KEY_LEN
)]
pub struct LineKeyError(String);

impl TryFrom<String> for LineKey {
    type Error = LineKeyError;

    fn try_from(key: String) -> Result<Self, Self::Error> {
        let valid = (1..=MAX_LINE_KEY_LEN).contains(&key.len())
            && key
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_');
        if valid {
            Ok(Self(key))
        } else {
            Err(LineKeyError(key))
        }
    }
}

impl std::str::FromStr for LineKey {
    type Err = LineKeyError;

    fn from_str(key: &str) -> Result<Self, Self::Err> {
        Self::try_from(key.to_owned())
    }
}

impl AsRef<str> for LineKey {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Lists of names and keys, written out by hand because a newtype over a `Vec` does not
/// derive to the array the wire carries.
#[cfg(feature = "schema")]
mod list_schema {
    use schemars::{
        JsonSchema,
        r#gen::SchemaGenerator,
        schema::{ArrayValidation, InstanceType, Schema, SchemaObject},
    };

    use super::{
        LineKey, LineKeys, MAX_DIMENSION_ENTRIES, MAX_HIDDEN_LINES, MetricFilter, MetricName,
    };

    fn array_of<T: JsonSchema>(generator: &mut SchemaGenerator, max_items: usize) -> Schema {
        SchemaObject {
            instance_type: Some(InstanceType::Array.into()),
            array: Some(Box::new(ArrayValidation {
                items: Some(generator.subschema_for::<T>().into()),
                max_items: u32::try_from(max_items).ok(),
                ..Default::default()
            })),
            ..Default::default()
        }
        .into()
    }

    impl JsonSchema for MetricFilter {
        fn schema_name() -> String {
            "MetricFilter".to_owned()
        }

        fn json_schema(generator: &mut SchemaGenerator) -> Schema {
            array_of::<MetricName>(generator, MAX_DIMENSION_ENTRIES)
        }
    }

    impl JsonSchema for LineKeys {
        fn schema_name() -> String {
            "LineKeys".to_owned()
        }

        fn json_schema(generator: &mut SchemaGenerator) -> Schema {
            array_of::<LineKey>(generator, MAX_HIDDEN_LINES)
        }
    }
}

/// Lists are stored as JSON text, and a line key as itself.
#[cfg(feature = "db")]
mod view_db {
    use diesel::{
        deserialize::{self, FromSql},
        serialize::{self, IsNull, Output, ToSql},
        sql_types::Text,
        sqlite::{Sqlite, SqliteValue},
    };

    use super::{LineKey, LineKeys, MetricFilter};

    macro_rules! json_text {
        ($name:ident) => {
            impl ToSql<Text, Sqlite> for $name {
                fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Sqlite>) -> serialize::Result {
                    out.set_value(serde_json::to_string(self)?);
                    Ok(IsNull::No)
                }
            }

            impl FromSql<Text, Sqlite> for $name {
                fn from_sql(mut bytes: SqliteValue<'_, '_, '_>) -> deserialize::Result<Self> {
                    Ok(serde_json::from_str(bytes.read_text())?)
                }
            }
        };
    }

    json_text!(MetricFilter);
    json_text!(LineKeys);

    impl ToSql<Text, Sqlite> for LineKey {
        fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Sqlite>) -> serialize::Result {
            out.set_value(self.0.as_str());
            Ok(IsNull::No)
        }
    }

    impl FromSql<Text, Sqlite> for LineKey {
        fn from_sql(mut bytes: SqliteValue<'_, '_, '_>) -> deserialize::Result<Self> {
            Ok(bytes.read_text().parse()?)
        }
    }
}

#[cfg(all(test, any(feature = "server", feature = "client")))]
mod tests {
    use super::{
        JsonUpdatePlot, LineKey, LineKeys, MAX_HIDDEN_LINES, MetricFilter, PlotLayout, XAxis, YAxis,
    };
    use crate::project::perf::MAX_DIMENSION_ENTRIES;

    #[test]
    fn y_axis_serde_round_trip() {
        for (y_axis, expected) in [
            (YAxis::Auto, "\"auto\""),
            (YAxis::Linear, "\"linear\""),
            (YAxis::Log, "\"log\""),
        ] {
            let json = serde_json::to_string(&y_axis).unwrap();
            assert_eq!(json, expected);
            let parsed: YAxis = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, y_axis);
        }
    }

    #[test]
    fn y_axis_default_is_auto() {
        assert_eq!(YAxis::default(), YAxis::Auto);
    }

    #[test]
    fn x_axis_serde_round_trip() {
        for (x_axis, expected) in [
            (XAxis::DateTime, "\"date_time\""),
            (XAxis::Version, "\"version\""),
        ] {
            let json = serde_json::to_string(&x_axis).unwrap();
            assert_eq!(json, expected);
            let parsed: XAxis = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, x_axis);
        }
    }

    #[test]
    fn deserialize_empty_is_patch_with_all_none() {
        let update: JsonUpdatePlot = serde_json::from_str("{}").unwrap();
        let JsonUpdatePlot::Patch(patch) = update else {
            panic!("expected Patch variant");
        };
        assert!(patch.index.is_none());
        assert!(patch.title.is_none());
        assert!(patch.lower_value.is_none());
        assert!(patch.upper_value.is_none());
        assert!(patch.lower_boundary.is_none());
        assert!(patch.upper_boundary.is_none());
        assert!(patch.x_axis.is_none());
        assert!(patch.y_axis.is_none());
        assert!(patch.window.is_none());
        assert!(patch.branches.is_none());
        assert!(patch.testbeds.is_none());
        assert!(patch.benchmarks.is_none());
        assert!(patch.parameters.is_none());
        assert!(patch.measures.is_none());
        assert!(patch.layout.is_none());
        assert!(patch.metrics.is_none());
        assert!(patch.hidden.is_none());
        assert!(patch.focus.is_none());
    }

    #[test]
    fn deserialize_title_string_is_patch() {
        let update: JsonUpdatePlot = serde_json::from_str(r#"{"title": "My Plot"}"#).unwrap();
        let JsonUpdatePlot::Patch(patch) = update else {
            panic!("expected Patch variant");
        };
        assert_eq!(patch.title.map(String::from), Some("My Plot".to_owned()));
    }

    #[test]
    fn deserialize_null_title_is_null_variant() {
        let update: JsonUpdatePlot =
            serde_json::from_str(r#"{"title": null, "lower_value": true}"#).unwrap();
        let JsonUpdatePlot::Null(patch) = update else {
            panic!("expected Null variant");
        };
        assert_eq!(patch.lower_value, Some(true));
    }

    #[test]
    fn deserialize_flags_and_axes() {
        let update: JsonUpdatePlot = serde_json::from_str(
            r#"{
                "lower_value": true,
                "upper_value": false,
                "lower_boundary": true,
                "upper_boundary": false,
                "x_axis": "version",
                "y_axis": "log"
            }"#,
        )
        .unwrap();
        let JsonUpdatePlot::Patch(patch) = update else {
            panic!("expected Patch variant");
        };
        assert_eq!(patch.lower_value, Some(true));
        assert_eq!(patch.upper_value, Some(false));
        assert_eq!(patch.lower_boundary, Some(true));
        assert_eq!(patch.upper_boundary, Some(false));
        assert!(matches!(patch.x_axis, Some(XAxis::Version)));
        assert_eq!(patch.y_axis, Some(YAxis::Log));
    }

    #[test]
    fn deserialize_component_lists() {
        let update: JsonUpdatePlot = serde_json::from_str(
            r#"{
                "branches": ["11111111-1111-1111-1111-111111111111"],
                "testbeds": ["22222222-2222-2222-2222-222222222222"],
                "benchmarks": [
                    "33333333-3333-3333-3333-333333333333",
                    "44444444-4444-4444-4444-444444444444"
                ],
                "measures": ["55555555-5555-5555-5555-555555555555"]
            }"#,
        )
        .unwrap();
        let JsonUpdatePlot::Patch(patch) = update else {
            panic!("expected Patch variant");
        };
        assert_eq!(patch.branches.as_ref().map(Vec::len), Some(1));
        assert_eq!(patch.testbeds.as_ref().map(Vec::len), Some(1));
        assert_eq!(patch.benchmarks.as_ref().map(Vec::len), Some(2));
        assert_eq!(patch.measures.as_ref().map(Vec::len), Some(1));
    }

    #[test]
    fn deserialize_empty_component_list_parses() {
        // An empty list is valid on the wire; the API layer rejects it (400)
        // since a plot must have at least one of each dimension.
        let update: JsonUpdatePlot = serde_json::from_str(r#"{"branches": []}"#).unwrap();
        let JsonUpdatePlot::Patch(patch) = update else {
            panic!("expected Patch variant");
        };
        assert_eq!(patch.branches.as_ref().map(Vec::len), Some(0));
    }

    #[test]
    fn deserialize_duplicate_field_errors() {
        serde_json::from_str::<JsonUpdatePlot>(r#"{"lower_value": true, "lower_value": false}"#)
            .unwrap_err();
    }

    #[test]
    fn deserialize_null_and_empty_parameters_both_clear() {
        for body in [r#"{"parameters": null}"#, r#"{"parameters": []}"#] {
            let update: JsonUpdatePlot = serde_json::from_str(body).unwrap();
            let JsonUpdatePlot::Patch(patch) = update else {
                panic!("expected Patch variant");
            };
            let parameters = patch.parameters.expect("parameters was written");
            assert!(parameters.is_match_all(), "{body}");
        }
    }

    #[test]
    fn deserialize_duplicate_parameters_field_errors() {
        serde_json::from_str::<JsonUpdatePlot>(r#"{"parameters": [], "parameters": []}"#)
            .unwrap_err();
    }

    #[test]
    fn deserialize_null_title_carries_parameters() {
        let update: JsonUpdatePlot =
            serde_json::from_str(r#"{"title": null, "parameters": [{"size": 1}]}"#).unwrap();
        let JsonUpdatePlot::Null(patch) = update else {
            panic!("expected Null variant");
        };
        let parameters = patch.parameters.expect("parameters was written");
        assert_eq!(parameters.canonical(), r#"[{"size":1}]"#);
    }

    #[test]
    fn deserialize_view_fields_set_and_clear() {
        let update: JsonUpdatePlot = serde_json::from_str(
            r#"{"metrics": ["p99"], "hidden": ["a1"], "focus": "a1", "layout": "stacked"}"#,
        )
        .unwrap();
        let JsonUpdatePlot::Patch(patch) = update else {
            panic!("expected Patch variant");
        };
        assert_eq!(patch.metrics.unwrap().names().len(), 1);
        assert_eq!(patch.hidden.unwrap().keys().len(), 1);
        assert_eq!(patch.focus, Some(Some("a1".parse().unwrap())));
        assert_eq!(patch.layout, Some(Some(PlotLayout::Stacked)));

        let update: JsonUpdatePlot = serde_json::from_str(
            r#"{"metrics": null, "hidden": null, "focus": null, "layout": null}"#,
        )
        .unwrap();
        let JsonUpdatePlot::Patch(patch) = update else {
            panic!("expected Patch variant");
        };
        assert!(patch.metrics.unwrap().is_empty());
        assert!(patch.hidden.unwrap().is_empty());
        assert_eq!(patch.focus, Some(None));
        assert_eq!(patch.layout, Some(None));
    }

    #[test]
    fn deserialize_null_title_carries_view_fields() {
        let update: JsonUpdatePlot = serde_json::from_str(
            r#"{"title": null, "metrics": ["p99"], "hidden": [], "focus": null, "layout": "dual"}"#,
        )
        .unwrap();
        let JsonUpdatePlot::Null(patch) = update else {
            panic!("expected Null variant");
        };
        assert_eq!(patch.metrics.unwrap().names().len(), 1);
        assert!(patch.hidden.unwrap().is_empty());
        assert_eq!(patch.focus, Some(None));
        assert_eq!(patch.layout, Some(Some(PlotLayout::Dual)));
    }

    #[test]
    fn deserialize_duplicate_view_field_errors() {
        for body in [
            r#"{"metrics": [], "metrics": []}"#,
            r#"{"hidden": [], "hidden": []}"#,
            r#"{"focus": null, "focus": null}"#,
            r#"{"layout": null, "layout": null}"#,
        ] {
            serde_json::from_str::<JsonUpdatePlot>(body).unwrap_err();
        }
    }

    #[test]
    fn line_key_is_url_safe() {
        for key in ["a", "0123456789abcdef", "A-_z"] {
            key.parse::<LineKey>().unwrap();
        }
        for key in ["", "0123456789abcdefg", "a/b", "a.b", "a b", "\u{e9}"] {
            key.parse::<LineKey>().unwrap_err();
        }
    }

    #[test]
    fn view_lists_cap_before_duplicates_collapse() {
        let names = |n: usize| serde_json::to_string(&vec!["p99"; n]).unwrap();
        let metrics: MetricFilter = serde_json::from_str(&names(MAX_DIMENSION_ENTRIES)).unwrap();
        assert_eq!(metrics.names().len(), 1);
        serde_json::from_str::<MetricFilter>(&names(MAX_DIMENSION_ENTRIES + 1)).unwrap_err();

        let keys = |n: usize| serde_json::to_string(&vec!["k"; n]).unwrap();
        let hidden: LineKeys = serde_json::from_str(&keys(MAX_HIDDEN_LINES)).unwrap();
        assert_eq!(hidden.keys().len(), 1);
        serde_json::from_str::<LineKeys>(&keys(MAX_HIDDEN_LINES + 1)).unwrap_err();

        let ordered: MetricFilter = serde_json::from_str(r#"["p99", "value", "p99"]"#).unwrap();
        assert_eq!(
            serde_json::to_string(&ordered).unwrap(),
            r#"["p99","value"]"#
        );
    }
}
