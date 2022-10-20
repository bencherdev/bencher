#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonConfig {
    #[serde(default = "default_url")]
    pub url: String,
    pub database: JsonDatabase,
    pub smtp: Option<JsonSmtp>,
}

fn default_url() -> String {
    #[cfg(debug_assertions)]
    {
        "http://localhost:3000".into()
    }
    #[cfg(not(debug_assertions))]
    {
        "https://bencher.dev".into()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonDatabase {
    pub path: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonSmtp {
    pub hostname: String,
    pub username: String,
    pub secret: String,
    pub from_name: String,
    pub from_email: String,
}
