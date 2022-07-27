#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{
    Deserialize,
    Serialize,
};
use url::Url;

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg(not(feature = "wasm"))]
pub struct JsonProject {
    pub name:        String,
    pub slug:        Option<String>,
    pub description: Option<String>,
    pub url:         Option<Url>,
    pub default:     bool,
}
