use bencher_valid::{NonEmpty, Sanitize, Secret, Url};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonOtel {
    /// The OTEL collector URL.
    pub endpoint: Url,
}
