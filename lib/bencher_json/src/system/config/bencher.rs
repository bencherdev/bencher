#![cfg(feature = "plus")]

use bencher_valid::{Sanitize, Secret};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonBencher {
    pub license_pem: Secret,
    pub billing_key: Secret,
}

impl Sanitize for JsonBencher {
    fn sanitize(&mut self) {
        self.license_pem.sanitize();
        self.billing_key.sanitize();
    }
}
