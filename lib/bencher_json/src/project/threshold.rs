use std::fmt;

use bencher_valid::{
    Boundary, BranchName, Confirmations, DateTime, DateTimeMillis, MetricName, Model, ModelTest,
    ResourceName, SampleSize, Significance, Window,
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
    /// The changepoint model, whose test checks a line, to check the threshold's lines with.
    /// If not set, the threshold has no changepoint model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changepoint_model: Option<Model>,
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
    /// The model whose test checks a line, when the threshold has one.
    pub changepoint_model: Option<JsonModel>,
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
    /// Absent when the threshold has no model, and so checks no metric.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<JsonConsoleThresholdRowModel>,
    /// Absent when the threshold has no changepoint model, and so checks no line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changepoint_model: Option<JsonConsoleThresholdRowModel>,
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
    /// Absent when the threshold has no model, and so checks no metric.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<JsonConsoleThresholdModel>,
    /// Every model the threshold has had, newest first, its current model included.
    pub models: Vec<JsonConsoleThresholdModel>,
    /// Absent when the threshold has no changepoint model, and so checks no line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changepoint_model: Option<JsonConsoleThresholdModel>,
    /// Every changepoint model the threshold has had, newest first, its current one included.
    pub changepoint_models: Vec<JsonConsoleThresholdModel>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub significance: Option<Significance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirmations: Option<Confirmations>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub significance: Option<Significance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirmations: Option<Confirmations>,
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
    ModelNull(JsonUpdateModelNull),
    RemoveNull(JsonRemoveModelNull),
}

impl JsonUpdateThreshold {
    /// The model to set, or none to remove it, and the changepoint model: absent for no
    /// change, `Some(None)` to remove it, or the changepoint model to set.
    #[must_use]
    pub fn into_models(self) -> (Option<Model>, Option<Option<Model>>) {
        match self {
            Self::Model(JsonUpdateModel {
                model,
                changepoint_model,
            }) => (Some(model), changepoint_model.map(Some)),
            Self::Remove(JsonRemoveModel {
                test: (),
                changepoint_model,
            }) => (None, changepoint_model.map(Some)),
            Self::ModelNull(JsonUpdateModelNull {
                model,
                changepoint_model: (),
            }) => (Some(model), Some(None)),
            Self::RemoveNull(JsonRemoveModelNull {
                test: (),
                changepoint_model: (),
            }) => (None, Some(None)),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUpdateModel {
    #[serde(flatten)]
    pub model: Model,
    /// The changepoint model, whose test checks a line.
    /// If not set, the changepoint model does not change.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changepoint_model: Option<Model>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonRemoveModel {
    pub test: (),
    /// The changepoint model, whose test checks a line.
    /// If not set, the changepoint model does not change.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changepoint_model: Option<Model>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUpdateModelNull {
    #[serde(flatten)]
    pub model: Model,
    /// Set to `null` to remove the changepoint model.
    pub changepoint_model: (),
}

#[derive(Debug, Clone, Copy, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonRemoveModelNull {
    pub test: (),
    /// Set to `null` to remove the changepoint model.
    pub changepoint_model: (),
}

impl<'de> Deserialize<'de> for JsonUpdateThreshold {
    #[expect(clippy::too_many_lines, reason = "one match arm per updatable field")]
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
        const SIGNIFICANCE_FIELD: &str = "significance";
        const CONFIRMATIONS_FIELD: &str = "confirmations";
        const CHANGEPOINT_MODEL_FIELD: &str = "changepoint_model";

        const FIELDS: &[&str] = &[
            TEST_FIELD,
            MIN_SAMPLE_SIZE_FIELD,
            MAX_SAMPLE_SIZE_FIELD,
            WINDOW_FIELD,
            LOWER_BOUNDARY_FIELD,
            UPPER_BOUNDARY_FIELD,
            SIGNIFICANCE_FIELD,
            CONFIRMATIONS_FIELD,
            CHANGEPOINT_MODEL_FIELD,
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
            Significance,
            Confirmations,
            ChangepointModel,
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
                let mut significance = None;
                let mut confirmations = None;
                let mut changepoint_model = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Test => {
                            next_once(&mut map, &mut test, TEST_FIELD)?;
                        },
                        Field::MinSampleSize => {
                            next_once(&mut map, &mut min_sample_size, MIN_SAMPLE_SIZE_FIELD)?;
                        },
                        Field::MaxSampleSize => {
                            next_once(&mut map, &mut max_sample_size, MAX_SAMPLE_SIZE_FIELD)?;
                        },
                        Field::Window => {
                            next_once(&mut map, &mut window, WINDOW_FIELD)?;
                        },
                        Field::LowerBoundary => {
                            next_once(&mut map, &mut lower_boundary, LOWER_BOUNDARY_FIELD)?;
                        },
                        Field::UpperBoundary => {
                            next_once(&mut map, &mut upper_boundary, UPPER_BOUNDARY_FIELD)?;
                        },
                        Field::Significance => {
                            next_once(&mut map, &mut significance, SIGNIFICANCE_FIELD)?;
                        },
                        Field::Confirmations => {
                            next_once(&mut map, &mut confirmations, CONFIRMATIONS_FIELD)?;
                        },
                        Field::ChangepointModel => {
                            next_once(&mut map, &mut changepoint_model, CHANGEPOINT_MODEL_FIELD)?;
                        },
                    }
                }

                let model = match test {
                    Some(Some(test)) => Some(Model {
                        test,
                        min_sample_size,
                        max_sample_size,
                        window,
                        lower_boundary,
                        upper_boundary,
                        significance,
                        confirmations,
                    }),
                    Some(None) => None,
                    None => return Err(de::Error::missing_field(TEST_FIELD)),
                };
                Ok(if let Some(None) = changepoint_model {
                    update_threshold_null(model)
                } else {
                    update_threshold(model, changepoint_model.flatten())
                })
            }
        }

        deserializer.deserialize_struct("JsonUpdateThreshold", FIELDS, UpdateThresholdVisitor)
    }
}

/// The update body that sets the model, or removes it when there is none, and sets the changepoint
/// model when there is one.
fn update_threshold(model: Option<Model>, changepoint_model: Option<Model>) -> JsonUpdateThreshold {
    if let Some(model) = model {
        JsonUpdateThreshold::Model(JsonUpdateModel {
            model,
            changepoint_model,
        })
    } else {
        JsonUpdateThreshold::Remove(JsonRemoveModel {
            test: (),
            changepoint_model,
        })
    }
}

/// The update body that sets the model, or removes it when there is none, and removes the
/// changepoint model.
fn update_threshold_null(model: Option<Model>) -> JsonUpdateThreshold {
    if let Some(model) = model {
        JsonUpdateThreshold::ModelNull(JsonUpdateModelNull {
            model,
            changepoint_model: (),
        })
    } else {
        JsonUpdateThreshold::RemoveNull(JsonRemoveModelNull {
            test: (),
            changepoint_model: (),
        })
    }
}

/// Read the value of a field, refusing a field the map has already given.
fn next_once<'de, V, T>(
    map: &mut V,
    field: &mut Option<T>,
    name: &'static str,
) -> Result<(), V::Error>
where
    V: de::MapAccess<'de>,
    T: Deserialize<'de>,
{
    if field.is_some() {
        return Err(de::Error::duplicate_field(name));
    }
    *field = Some(map.next_value()?);
    Ok(())
}
