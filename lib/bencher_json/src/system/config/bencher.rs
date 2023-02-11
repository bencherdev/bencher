#![cfg(feature = "plus")]

use bencher_valid::{Sanitize, Secret};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonBencher {
    pub private_pem: Secret,
}

impl Sanitize for JsonBencher {
    fn sanitize(&mut self) {
        self.private_pem.sanitize();
    }
}
