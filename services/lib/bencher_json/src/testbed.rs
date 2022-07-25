#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{
    Deserialize,
    Serialize,
};
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg(not(feature = "wasm"))]
pub struct JsonTestbed {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ram: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disk: Option<String>,
}

#[cfg(feature = "wasm")]
#[wasm_bindgen]
pub struct JsonTestbed {
    name: String,
    os_name: Option<String>,
    os_version: Option<String>,
    runtime_name: Option<String>,
    runtime_version: Option<String>,
    cpu: Option<String>,
    ram: Option<String>,
    disk: Option<String>,
}

#[cfg(feature = "wasm")]
#[wasm_bindgen]
impl JsonTestbed {
    #[cfg_attr(feature = "wasm", wasm_bindgen(getter))]
    pub fn name(&self) -> String {
        self.name.clone()
    }

    #[cfg_attr(feature = "wasm", wasm_bindgen(getter))]
    pub fn os_name(&self) -> Option<String> {
        self.os_name.clone()
    }

    #[cfg_attr(feature = "wasm", wasm_bindgen(getter))]
    pub fn os_version(&self) -> Option<String> {
        self.os_version.clone()
    }

    #[cfg_attr(feature = "wasm", wasm_bindgen(getter))]
    pub fn runtime_name(&self) -> Option<String> {
        self.runtime_name.clone()
    }

    #[cfg_attr(feature = "wasm", wasm_bindgen(getter))]
    pub fn runtime_version(&self) -> Option<String> {
        self.runtime_version.clone()
    }

    #[cfg_attr(feature = "wasm", wasm_bindgen(getter))]
    pub fn cpu(&self) -> Option<String> {
        self.cpu.clone()
    }

    #[cfg_attr(feature = "wasm", wasm_bindgen(getter))]
    pub fn ram(&self) -> Option<String> {
        self.ram.clone()
    }

    #[cfg_attr(feature = "wasm", wasm_bindgen(getter))]
    pub fn disk(&self) -> Option<String> {
        self.disk.clone()
    }
}
