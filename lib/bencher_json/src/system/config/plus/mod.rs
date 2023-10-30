#![cfg(feature = "plus")]

use bencher_valid::Sanitize;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub mod cloud;

pub use cloud::JsonCloud;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPlus {
    pub cloud: Option<JsonCloud>,
}

impl Sanitize for JsonPlus {
    fn sanitize(&mut self) {
        self.cloud.sanitize();
    }
}
