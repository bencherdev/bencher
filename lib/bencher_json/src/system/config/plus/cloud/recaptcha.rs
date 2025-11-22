use bencher_valid::{RecaptchaScore, Sanitize, Secret};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonRecaptcha {
    /// The shared key between your site and reCAPTCHA.
    pub secret: Secret,
    /// The minimum acceptable score for reCAPTCHA verification.
    pub min_score: RecaptchaScore,
}

impl Sanitize for JsonRecaptcha {
    fn sanitize(&mut self) {
        self.secret.sanitize();
    }
}
