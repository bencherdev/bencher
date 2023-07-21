#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::JsonEmpty;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(untagged, rename_all = "snake_case")]
pub enum JsonNullable<T> {
    Value(T),
    Null(JsonEmpty),
}

impl<T> From<JsonNullable<T>> for Option<T> {
    fn from(nullable: JsonNullable<T>) -> Self {
        match nullable {
            JsonNullable::Value(value) => Some(value),
            JsonNullable::Null(_) => None,
        }
    }
}
