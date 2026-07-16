use std::fmt;

use bencher_valid::{DateTime, Index, ResourceName, Window};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{self, Visitor},
};

use crate::{BenchmarkUuid, BranchUuid, MeasureUuid, ProjectUuid, TestbedUuid};

crate::typed_uuid::typed_uuid!(PlotUuid);

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
    pub window: Window,
    pub branches: Vec<BranchUuid>,
    pub testbeds: Vec<TestbedUuid>,
    pub benchmarks: Vec<BenchmarkUuid>,
    pub measures: Vec<MeasureUuid>,
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
    /// The window of time for the plot, in seconds.
    /// Metrics outside of this window will be omitted.
    pub window: Option<Window>,
    /// The branches to include in the plot.
    /// Replaces the current branches for the plot.
    /// At least one branch must be specified.
    pub branches: Option<Vec<BranchUuid>>,
    /// The testbeds to include in the plot.
    /// Replaces the current testbeds for the plot.
    /// At least one testbed must be specified.
    pub testbeds: Option<Vec<TestbedUuid>>,
    /// The benchmarks to include in the plot.
    /// Replaces the current benchmarks for the plot.
    /// At least one benchmark must be specified.
    pub benchmarks: Option<Vec<BenchmarkUuid>>,
    /// The measures to include in the plot.
    /// Replaces the current measures for the plot.
    /// At least one measure must be specified.
    pub measures: Option<Vec<MeasureUuid>>,
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
    pub window: Option<Window>,
    pub branches: Option<Vec<BranchUuid>>,
    pub testbeds: Option<Vec<TestbedUuid>>,
    pub benchmarks: Option<Vec<BenchmarkUuid>>,
    pub measures: Option<Vec<MeasureUuid>>,
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
        const WINDOW_FIELD: &str = "window";
        const BRANCHES_FIELD: &str = "branches";
        const TESTBEDS_FIELD: &str = "testbeds";
        const BENCHMARKS_FIELD: &str = "benchmarks";
        const MEASURES_FIELD: &str = "measures";
        const FIELDS: &[&str] = &[
            INDEX_FIELD,
            TITLE_FIELD,
            LOWER_VALUE_FIELD,
            UPPER_VALUE_FIELD,
            LOWER_BOUNDARY_FIELD,
            UPPER_BOUNDARY_FIELD,
            X_AXIS_FIELD,
            WINDOW_FIELD,
            BRANCHES_FIELD,
            TESTBEDS_FIELD,
            BENCHMARKS_FIELD,
            MEASURES_FIELD,
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
            Window,
            Branches,
            Testbeds,
            Benchmarks,
            Measures,
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
                let mut window = None;
                let mut branches = None;
                let mut testbeds = None;
                let mut benchmarks = None;
                let mut measures = None;

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
                        Field::Measures => {
                            if measures.is_some() {
                                return Err(de::Error::duplicate_field(MEASURES_FIELD));
                            }
                            measures = Some(map.next_value()?);
                        },
                    }
                }

                Ok(match title {
                    Some(Some(title)) => Self::Value::Patch(JsonPlotPatch {
                        index,
                        title: Some(title),
                        lower_value,
                        upper_value,
                        lower_boundary,
                        upper_boundary,
                        x_axis,
                        window,
                        branches,
                        testbeds,
                        benchmarks,
                        measures,
                    }),
                    Some(None) => Self::Value::Null(JsonPlotPatchNull {
                        index,
                        title: (),
                        lower_value,
                        upper_value,
                        lower_boundary,
                        upper_boundary,
                        x_axis,
                        window,
                        branches,
                        testbeds,
                        benchmarks,
                        measures,
                    }),
                    None => Self::Value::Patch(JsonPlotPatch {
                        index,
                        title: None,
                        lower_value,
                        upper_value,
                        lower_boundary,
                        upper_boundary,
                        x_axis,
                        window,
                        branches,
                        testbeds,
                        benchmarks,
                        measures,
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
}

pub const LOWER_VALUE: &str = "lower_value";
pub const UPPER_VALUE: &str = "upper_value";
pub const LOWER_BOUNDARY: &str = "lower_boundary";
pub const UPPER_BOUNDARY: &str = "upper_boundary";
pub const X_AXIS: &str = "x_axis";

const DATE_TIME_INT: i32 = 0;
const VERSION_INT: i32 = 1;

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

#[cfg(all(test, any(feature = "server", feature = "client")))]
mod tests {
    use super::{JsonUpdatePlot, XAxis};

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
        assert!(patch.window.is_none());
        assert!(patch.branches.is_none());
        assert!(patch.testbeds.is_none());
        assert!(patch.benchmarks.is_none());
        assert!(patch.measures.is_none());
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
    fn deserialize_flags_and_x_axis() {
        let update: JsonUpdatePlot = serde_json::from_str(
            r#"{
                "lower_value": true,
                "upper_value": false,
                "lower_boundary": true,
                "upper_boundary": false,
                "x_axis": "version"
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
}
