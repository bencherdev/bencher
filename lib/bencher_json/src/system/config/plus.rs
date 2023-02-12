#![cfg(feature = "plus")]

use bencher_valid::{Sanitize, Secret};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonPlus {
    pub billing: JsonBilling,
    pub license_pem: Secret,
}

impl Sanitize for JsonPlus {
    fn sanitize(&mut self) {
        self.billing.sanitize();
        self.license_pem.sanitize();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonBilling {
    pub secret_key: Secret,
}

impl Sanitize for JsonBilling {
    fn sanitize(&mut self) {
        self.secret_key.sanitize();
    }
}
