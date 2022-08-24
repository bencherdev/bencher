use ordered_float::OrderedFloat;
use schemars::JsonSchema;
use serde::{
    Deserialize,
    Serialize,
};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonAlert {
    pub uuid:      Uuid,
    pub perf:      Uuid,
    pub threshold: Uuid,
    pub statistic: Uuid,
    pub boundary:  OrderedFloat<f64>,
    pub outlier:   OrderedFloat<f64>,
}
