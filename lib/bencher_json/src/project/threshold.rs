use std::fmt;

use bencher_valid::{
    Boundary, BranchName, DateTime, DateTimeMillis, MetricName, Model, ModelTest, ResourceName,
    SampleSize, Window,
};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{self, Visitor},
};

use crate::{
    BranchNameId, BranchSlug, BranchUuid, JsonBranch, JsonMeasure, JsonModel, JsonTestbed,
    MeasureNameId, MeasureSlug, MeasureUuid, ModelUuid, ParameterFilter, ProjectUuid,
    TestbedNameId, TestbedSlug, TestbedUuid,
    urlencoded::{UrlEncodedError, from_urlencoded, to_urlencoded},
};

crate::typed_uuid::typed_uuid!(ThresholdUuid);

/// The most thresholds with a model one branch, testbed, and measure may have.
pub const MAX_ACTIVE_THRESHOLDS: usize = 64;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewThreshold {
    /// The UUID, slug, or name of the threshold branch.
    pub branch: BranchNameId,
    /// The UUID, slug, or name of the threshold testbed.
    pub testbed: TestbedNameId,
    /// The variants this threshold checks, as a parameters filter.
    /// A variant matches when any entry in the filter is a subset of its parameters.
    /// If not set, or set to an empty list, the threshold checks every variant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<ParameterFilter>,
    /// The UUID, slug, or name of the threshold measure.
    pub measure: MeasureNameId,
    /// The name of the metric this threshold checks.
    /// If not set, the threshold checks the conventional `value` name.
    /// A threshold always checks exactly one name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metric: Option<MetricName>,
    #[serde(flatten)]
    pub model: Model,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonThresholds(pub Vec<JsonThreshold>);

crate::from_vec!(JsonThresholds[JsonThreshold]);

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonThreshold {
    pub uuid: ThresholdUuid,
    pub project: ProjectUuid,
    pub branch: JsonBranch,
    pub testbed: JsonTestbed,
    /// The variants this threshold checks, in canonical order.
    /// Absent when the threshold checks every variant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<ParameterFilter>,
    pub measure: JsonMeasure,
    /// The name of the metric this threshold checks.
    /// Absent when the threshold checks the conventional `value` name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metric: Option<MetricName>,
    pub model: Option<JsonModel>,
    pub created: DateTime,
    pub modified: DateTime,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonThresholdModel {
    pub uuid: ThresholdUuid,
    pub project: ProjectUuid,
    /// The variants this threshold checks, in canonical order.
    /// Absent when the threshold checks every variant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<ParameterFilter>,
    /// The name of the metric this threshold checks.
    /// Absent when the threshold checks the conventional `value` name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metric: Option<MetricName>,
    pub model: JsonModel,
    pub created: DateTime,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonThresholdQueryParams {
    /// Filter by branch name, exact match.
    pub branch: Option<String>,
    /// Filter by testbed name, exact match.
    pub testbed: Option<String>,
    /// Filter by measure name, exact match.
    pub measure: Option<String>,
    /// If set to `true`, only return thresholds with an archived branch, testbed, or measure.
    /// If not set or set to `false`, only returns thresholds with non-archived branches, testbeds, and measures.
    pub archived: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct JsonThresholdQuery {
    pub branch: Option<BranchNameId>,
    pub testbed: Option<TestbedNameId>,
    pub measure: Option<MeasureNameId>,
    pub archived: Option<bool>,
}

impl TryFrom<JsonThresholdQueryParams> for JsonThresholdQuery {
    type Error = UrlEncodedError;

    fn try_from(query_params: JsonThresholdQueryParams) -> Result<Self, Self::Error> {
        let JsonThresholdQueryParams {
            branch,
            testbed,
            measure,
            archived,
        } = query_params;

        let branch = if let Some(branch) = branch {
            Some(from_urlencoded(&branch)?)
        } else {
            None
        };
        let testbed = if let Some(testbed) = testbed {
            Some(from_urlencoded(&testbed)?)
        } else {
            None
        };
        let measure = if let Some(measure) = measure {
            Some(from_urlencoded(&measure)?)
        } else {
            None
        };

        Ok(Self {
            branch,
            testbed,
            measure,
            archived,
        })
    }
}

impl JsonThresholdQuery {
    pub fn branch(&self) -> Option<String> {
        self.branch.as_ref().map(to_urlencoded)
    }

    pub fn testbed(&self) -> Option<String> {
        self.testbed.as_ref().map(to_urlencoded)
    }

    pub fn measure(&self) -> Option<String> {
        self.measure.as_ref().map(to_urlencoded)
    }
}

/// How many thresholds a page of the console's list holds when the request does not say.
pub const DEFAULT_CONSOLE_THRESHOLDS_PER_PAGE: u8 = 64;

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleThresholdsQueryParams {
    /// Only the thresholds on this branch.
    pub branch: Option<BranchUuid>,
    /// Only the thresholds on this testbed.
    pub testbed: Option<TestbedUuid>,
    /// Only the thresholds on this measure.
    pub measure: Option<MeasureUuid>,
    /// If set to `true`, only the thresholds with an archived branch, testbed, or measure.
    /// Otherwise, only the thresholds with none of them archived.
    pub archived: Option<bool>,
    /// The earliest time a counted alert was raised, in milliseconds, inclusive.
    pub start_time: Option<DateTimeMillis>,
    /// The latest time a counted alert was raised, in milliseconds, inclusive.
    pub end_time: Option<DateTimeMillis>,
    /// The page of thresholds to return, starting at 1.
    pub page: Option<u32>,
    /// The number of thresholds per page, 64 when not given.
    pub per_page: Option<u8>,
}

/// A page of a project's thresholds as the console lists them, oldest first, with the
/// branches, testbeds, and measures they apply to once each.
#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleThresholds {
    /// The number of thresholds that match the filters, over every page.
    pub total: u32,
    pub thresholds: Vec<JsonConsoleThresholdRow>,
    pub branches: Vec<JsonConsoleThresholdBranch>,
    pub testbeds: Vec<JsonConsoleThresholdTestbed>,
    pub measures: Vec<JsonConsoleThresholdMeasure>,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleThresholdRow {
    pub uuid: ThresholdUuid,
    /// An index into `branches`.
    pub branch: u32,
    /// An index into `testbeds`.
    pub testbed: u32,
    /// An index into `measures`.
    pub measure: u32,
    /// The variants this threshold checks, in canonical order.
    /// Absent when the threshold checks every variant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<ParameterFilter>,
    /// The name of the metric this threshold checks.
    /// Absent when the threshold checks the conventional `value` name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metric: Option<MetricName>,
    /// Absent when the threshold has no model, and so checks nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<JsonConsoleThresholdRowModel>,
    /// The alerts the threshold raised inside the window, whatever their status now.
    pub raised: u32,
    /// The threshold's alerts that are active now, whenever they were raised.
    pub active: u32,
}

/// One threshold as the console's threshold page draws it.
///
/// The report that declared a threshold or set a model is not recorded.
#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleThreshold {
    pub uuid: ThresholdUuid,
    pub branch: JsonConsoleThresholdBranch,
    pub testbed: JsonConsoleThresholdTestbed,
    pub measure: JsonConsoleThresholdMeasure,
    /// The variants this threshold checks, in canonical order.
    /// Absent when the threshold checks every variant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<ParameterFilter>,
    /// The name of the metric this threshold checks.
    /// Absent when the threshold checks the conventional `value` name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metric: Option<MetricName>,
    /// Absent when the threshold has no model, and so checks nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<JsonConsoleThresholdModel>,
    /// Every model the threshold has had, newest first, its current model included.
    pub models: Vec<JsonConsoleThresholdModel>,
    pub created: DateTimeMillis,
    pub modified: DateTimeMillis,
}

/// A threshold's current model as the list draws it: its test and that test's parameters.
#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleThresholdRowModel {
    pub test: ModelTest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_sample_size: Option<SampleSize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_sample_size: Option<SampleSize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<Window>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lower_boundary: Option<Boundary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upper_boundary: Option<Boundary>,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleThresholdModel {
    pub uuid: ModelUuid,
    pub test: ModelTest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_sample_size: Option<SampleSize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_sample_size: Option<SampleSize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<Window>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lower_boundary: Option<Boundary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upper_boundary: Option<Boundary>,
    pub created: DateTimeMillis,
    /// When a newer model, or the removal of the threshold's model, replaced it.
    /// Absent for the current model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replaced: Option<DateTimeMillis>,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleThresholdBranch {
    pub uuid: BranchUuid,
    pub name: BranchName,
    pub slug: BranchSlug,
    /// The name of the branch that this branch's current head started from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_point: Option<BranchName>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived: Option<DateTimeMillis>,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleThresholdTestbed {
    pub uuid: TestbedUuid,
    pub name: ResourceName,
    pub slug: TestbedSlug,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived: Option<DateTimeMillis>,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConsoleThresholdMeasure {
    pub uuid: MeasureUuid,
    pub name: ResourceName,
    pub slug: MeasureSlug,
    pub units: ResourceName,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived: Option<DateTimeMillis>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(untagged)]
pub enum JsonUpdateThreshold {
    Model(JsonUpdateModel),
    Remove(JsonRemoveModel),
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUpdateModel {
    #[serde(flatten)]
    pub model: Model,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonRemoveModel {
    pub test: (),
}

impl<'de> Deserialize<'de> for JsonUpdateThreshold {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        const TEST_FIELD: &str = "test";
        const MIN_SAMPLE_SIZE_FIELD: &str = "min_sample_size";
        const MAX_SAMPLE_SIZE_FIELD: &str = "max_sample_size";
        const WINDOW_FIELD: &str = "window";
        const LOWER_BOUNDARY_FIELD: &str = "lower_boundary";
        const UPPER_BOUNDARY_FIELD: &str = "upper_boundary";

        const FIELDS: &[&str] = &[
            TEST_FIELD,
            MIN_SAMPLE_SIZE_FIELD,
            MAX_SAMPLE_SIZE_FIELD,
            WINDOW_FIELD,
            LOWER_BOUNDARY_FIELD,
            UPPER_BOUNDARY_FIELD,
        ];

        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field {
            Test,
            MinSampleSize,
            MaxSampleSize,
            Window,
            LowerBoundary,
            UpperBoundary,
        }

        struct UpdateThresholdVisitor;

        impl<'de> Visitor<'de> for UpdateThresholdVisitor {
            type Value = JsonUpdateThreshold;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("JsonUpdateThreshold")
            }

            fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
            where
                V: de::MapAccess<'de>,
            {
                let mut test = None;
                let mut min_sample_size = None;
                let mut max_sample_size = None;
                let mut window = None;
                let mut lower_boundary = None;
                let mut upper_boundary = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Test => {
                            if test.is_some() {
                                return Err(de::Error::duplicate_field(TEST_FIELD));
                            }
                            test = Some(map.next_value()?);
                        },
                        Field::MinSampleSize => {
                            if min_sample_size.is_some() {
                                return Err(de::Error::duplicate_field(MIN_SAMPLE_SIZE_FIELD));
                            }
                            min_sample_size = Some(map.next_value()?);
                        },
                        Field::MaxSampleSize => {
                            if max_sample_size.is_some() {
                                return Err(de::Error::duplicate_field(MAX_SAMPLE_SIZE_FIELD));
                            }
                            max_sample_size = Some(map.next_value()?);
                        },
                        Field::Window => {
                            if window.is_some() {
                                return Err(de::Error::duplicate_field(WINDOW_FIELD));
                            }
                            window = Some(map.next_value()?);
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
                    }
                }

                match test {
                    Some(Some(test)) => Ok(Self::Value::Model(JsonUpdateModel {
                        model: Model {
                            test,
                            min_sample_size,
                            max_sample_size,
                            window,
                            lower_boundary,
                            upper_boundary,
                        },
                    })),
                    Some(None) => Ok(Self::Value::Remove(JsonRemoveModel { test: () })),
                    None => Err(de::Error::missing_field(TEST_FIELD)),
                }
            }
        }

        deserializer.deserialize_struct("JsonUpdateThreshold", FIELDS, UpdateThresholdVisitor)
    }
}
