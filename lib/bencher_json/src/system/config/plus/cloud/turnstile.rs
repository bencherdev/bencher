use bencher_valid::{Sanitize, Secret};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonTurnstile {
    /// The secret key of the Cloudflare Turnstile widget.
    pub secret: Secret,
}

impl Sanitize for JsonTurnstile {
    fn sanitize(&mut self) {
        self.secret.sanitize();
    }
}
