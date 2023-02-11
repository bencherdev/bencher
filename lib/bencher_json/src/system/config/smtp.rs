use bencher_valid::{Sanitize, Secret};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonSmtp {
    pub hostname: String,
    pub port: Option<u16>,
    pub starttls: Option<bool>,
    pub username: String,
    pub secret: Secret,
    pub from_name: String,
    pub from_email: String,
}

impl Sanitize for JsonSmtp {
    fn sanitize(&mut self) {
        self.secret.sanitize();
    }
}
