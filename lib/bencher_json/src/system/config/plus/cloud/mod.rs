use bencher_valid::{Sanitize, Secret};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub mod billing;
pub mod index;
pub mod otel;
pub mod recaptcha;
pub mod turnstile;

use billing::JsonBilling;
use index::JsonIndex;
use otel::JsonOtel;
use recaptcha::JsonRecaptcha;
use turnstile::JsonTurnstile;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonCloud {
    pub billing: JsonBilling,
    pub license_pem: Secret,
    pub sentry: Option<Secret>,
    pub otel: Option<JsonOtel>,
    pub index: Option<JsonIndex>,
    pub recaptcha: Option<JsonRecaptcha>,
    pub turnstile: Option<JsonTurnstile>,
}

impl Sanitize for JsonCloud {
    fn sanitize(&mut self) {
        self.billing.sanitize();
        self.license_pem.sanitize();
        self.sentry.sanitize();
        self.index.sanitize();
        self.recaptcha.sanitize();
        self.turnstile.sanitize();
    }
}
