#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonOpenApiSpec(pub serde_json::Value);

impl JsonOpenApiSpec {
    pub fn version(&self) -> Option<&str> {
        self.0
            .get("info")
            .and_then(|info| info.get("version"))
            .and_then(serde_json::Value::as_str)
    }
}
