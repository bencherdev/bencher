use bencher_valid::{Boundary, DateTime, ModelTest, SampleSize, Window};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ThresholdUuid;

crate::typed_uuid::typed_uuid!(ModelUuid);

#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonModel {
    pub uuid: ModelUuid,
    pub threshold: ThresholdUuid,
    pub test: ModelTest,
    pub min_sample_size: Option<SampleSize>,
    pub max_sample_size: Option<SampleSize>,
    pub window: Option<Window>,
    pub lower_boundary: Option<Boundary>,
    pub upper_boundary: Option<Boundary>,
    pub created: DateTime,
    pub replaced: Option<DateTime>,
}
