use bencher_valid::{Email, NonEmpty, Sanitize, Secret};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonSmtp {
    pub hostname: NonEmpty,
    pub port: Option<u16>,
    pub starttls: Option<bool>,
    pub username: NonEmpty,
    pub secret: Secret,
    pub from_name: NonEmpty,
    pub from_email: Email,
}

impl Sanitize for JsonSmtp {
    fn sanitize(&mut self) {
        self.secret.sanitize();
    }
}
