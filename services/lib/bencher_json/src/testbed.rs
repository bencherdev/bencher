#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{
    Deserialize,
    Serialize,
};

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonTestbed {
    pub name:       String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os_name:    Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu:        Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ram:        Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disk:       Option<String>,
}

impl JsonTestbed {
    pub fn new(
        name: String,
        os_name: Option<String>,
        os_version: Option<String>,
        cpu: Option<String>,
        ram: Option<String>,
        disk: Option<String>,
    ) -> Self {
        Self {
            name,
            os_name,
            os_version,
            cpu,
            ram,
            disk,
        }
    }
}
