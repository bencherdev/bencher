use bencher_valid::DateTime;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{BranchUuid, JsonStartPoint};

crate::typed_uuid::typed_uuid!(ReferenceUuid);

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonReference {
    pub uuid: ReferenceUuid,
    pub branch: BranchUuid,
    pub start_point: Option<JsonStartPoint>,
    pub created: DateTime,
    pub removed: Option<DateTime>,
}
