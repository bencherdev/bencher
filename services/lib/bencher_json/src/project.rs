#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{
    Deserialize,
    Serialize,
};
use url::Url;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg(not(feature = "wasm"))]
pub struct JsonNewProject {
    pub name:        String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug:        Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url:         Option<Url>,
    pub default:     bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg(not(feature = "wasm"))]
pub struct JsonProject {
    pub uuid:          Uuid,
    pub owner_uuid:    Uuid,
    pub owner_default: bool,
    pub name:          String,
    pub slug:          String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description:   Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url:           Option<Url>,
}
