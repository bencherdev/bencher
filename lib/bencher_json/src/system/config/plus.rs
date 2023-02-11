#![cfg(feature = "plus")]

use bencher_valid::{Sanitize, Secret};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPlus {
    pub billing_key: Secret,
    pub license_pem: Secret,
}

impl Sanitize for JsonPlus {
    fn sanitize(&mut self) {
        self.billing_key.sanitize();
        self.license_pem.sanitize();
    }
}
