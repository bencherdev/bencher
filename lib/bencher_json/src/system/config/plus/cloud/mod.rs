use bencher_valid::{Sanitize, Secret};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub mod billing;
pub mod index;

use billing::JsonBilling;
use index::JsonIndex;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonCloud {
    pub billing: JsonBilling,
    pub license_pem: Secret,
    pub sentry: Option<Secret>,
    pub index: Option<JsonIndex>,
}

impl Sanitize for JsonCloud {
    fn sanitize(&mut self) {
        self.billing.sanitize();
        self.license_pem.sanitize();
        self.sentry.sanitize();
        self.index.sanitize();
    }
}
