#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewClaim {
    // This is a bit of a kludge to make this an empty object
    #[serde(skip_serializing_if = "Option::is_none")]
    pub empty: Option<()>,
}
