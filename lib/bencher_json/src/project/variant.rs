use std::fmt;

use bencher_valid::DateTime;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{BenchmarkUuid, ParameterSet, ParameterSetInput};

crate::typed_uuid::typed_uuid!(VariantUuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewVariant {
    /// The parameters of the variant.
    /// Each key maps to a JSON scalar: a string, a number, or a boolean.
    pub parameters: ParameterSetInput,
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

#[cfg(test)]
mod tests {
    use super::{JsonNewVariant, JsonVariant};
    use crate::MAX_PARAMETER_KEYS;

    /// Kills a new variant read without the key cap, and a variant response read with it.
    #[test]
    fn new_variant_caps_keys_and_variant_reads_past_them() {
        serde_json::from_value::<JsonNewVariant>(
            serde_json::json!({ "parameters": keys(MAX_PARAMETER_KEYS) }),
        )
        .expect("Failed to read a new variant at the cap");
        serde_json::from_value::<JsonNewVariant>(
            serde_json::json!({ "parameters": keys(MAX_PARAMETER_KEYS + 1) }),
        )
        .unwrap_err();

        let variant = serde_json::from_value::<JsonVariant>(serde_json::json!({
            "uuid": "00000000-0000-0000-0000-000000000001",
            "benchmark": "00000000-0000-0000-0000-000000000002",
            "parameters": keys(MAX_PARAMETER_KEYS * 2),
            "created": "2026-01-01T00:00:00Z",
            "modified": "2026-01-01T00:00:00Z",
            "archived": null,
        }))
        .expect("Failed to read a variant past the cap");
        assert_eq!(variant.parameters.tags().count(), MAX_PARAMETER_KEYS * 2);
    }

    fn keys(count: usize) -> serde_json::Map<String, serde_json::Value> {
        (0..count)
            .map(|key| (format!("k{key}"), serde_json::Value::from(key)))
            .collect()
    }
}
