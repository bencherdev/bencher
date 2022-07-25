#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{
    Deserialize,
    Serialize,
};
use uuid::Uuid;
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg(not(feature = "wasm"))]
pub struct JsonSignup {
    pub name:  String,
    pub slug:  Option<String>,
    pub email: String,
}

#[cfg(feature = "wasm")]
#[wasm_bindgen]
pub struct JsonSignup {
    name:  String,
    slug:  Option<String>,
    email: String,
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
impl JsonSignup {
    #[cfg_attr(feature = "wasm", wasm_bindgen(getter))]
    pub fn name(&self) -> String {
        self.name.clone()
    }

    #[cfg_attr(feature = "wasm", wasm_bindgen(getter))]
    pub fn slug(&self) -> Option<String> {
        self.slug.clone()
    }

    #[cfg_attr(feature = "wasm", wasm_bindgen(getter))]
    pub fn email(&self) -> String {
        self.email.clone()
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg(not(feature = "wasm"))]
pub struct JsonUser {
    pub uuid:  Uuid,
    pub name:  String,
    pub slug:  String,
    pub email: String,
}

#[cfg(feature = "wasm")]
#[wasm_bindgen]
pub struct JsonUser {
    uuid:  Uuid,
    name:  String,
    slug:  String,
    email: String,
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
impl JsonUser {
    #[cfg_attr(feature = "wasm", wasm_bindgen(getter))]
    pub fn uuid(&self) -> Uuid {
        self.uuid.clone()
    }

    #[cfg_attr(feature = "wasm", wasm_bindgen(getter))]
    pub fn name(&self) -> String {
        self.name.clone()
    }

    #[cfg_attr(feature = "wasm", wasm_bindgen(getter))]
    pub fn slug(&self) -> String {
        self.slug.clone()
    }

    #[cfg_attr(feature = "wasm", wasm_bindgen(getter))]
    pub fn email(&self) -> String {
        self.email.clone()
    }
}
