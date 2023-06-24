use ordered_float::OrderedFloat;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonBoundary {
    pub left_side: Option<OrderedFloat<f64>>,
    pub right_side: Option<OrderedFloat<f64>>,
}
