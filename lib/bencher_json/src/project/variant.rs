use std::fmt;

use bencher_valid::DateTime;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{BenchmarkUuid, ParameterSet};

crate::typed_uuid::typed_uuid!(VariantUuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewVariant {
    /// The parameters of the variant.
    /// Each key maps to a JSON scalar: a string, a number, or a boolean.
    pub parameters: ParameterSet,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonVariants(pub Vec<JsonVariant>);

crate::from_vec!(JsonVariants[JsonVariant]);

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonVariant {
    pub uuid: VariantUuid,
    pub benchmark: BenchmarkUuid,
    pub parameters: ParameterSet,
    pub created: DateTime,
    pub modified: DateTime,
    pub archived: Option<DateTime>,
}

impl fmt::Display for JsonVariant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.parameters)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUpdateVariant {
    /// Set whether the variant is archived.
    pub archived: Option<bool>,
}
