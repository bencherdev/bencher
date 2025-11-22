use bencher_valid::{Sanitize, Secret};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub mod billing;
pub mod index;
pub mod otel;
pub mod recaptcha;

use billing::JsonBilling;
use index::JsonIndex;
use otel::JsonOtel;
use recaptcha::JsonRecaptcha;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonCloud {
    pub billing: JsonBilling,
    pub license_pem: Secret,
    pub sentry: Option<Secret>,
    pub otel: Option<JsonOtel>,
    pub index: Option<JsonIndex>,
    pub recaptcha: Option<JsonRecaptcha>,
}

impl Sanitize for JsonCloud {
    fn sanitize(&mut self) {
        self.billing.sanitize();
        self.license_pem.sanitize();
        self.sentry.sanitize();
        self.index.sanitize();
        self.recaptcha.sanitize();
    }
}
