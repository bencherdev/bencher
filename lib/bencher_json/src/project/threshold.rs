use std::fmt;

use bencher_valid::{DateTime, Model, NameId};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer, Serialize,
};

use crate::{
    urlencoded::{from_urlencoded, to_urlencoded, UrlEncodedError},
    JsonBranch, JsonMeasure, JsonModel, JsonTestbed, ProjectUuid,
};

crate::typed_uuid::typed_uuid!(ThresholdUuid);

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewThreshold {
    /// The UUID, slug, or name of the threshold branch.
    pub branch: NameId,
    /// The UUID, slug, or name of the threshold testbed.
    pub testbed: NameId,
    /// The UUID, slug, or name of the threshold measure.
    pub measure: NameId,
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
    pub measure: JsonMeasure,
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
    pub branch: Option<NameId>,
    pub testbed: Option<NameId>,
    pub measure: Option<NameId>,
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
                formatter.write_str("JsonUpdatePlot")
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
