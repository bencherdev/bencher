use bencher_valid::{Email, ResourceName, Sanitize, Secret};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonSmtp {
    pub hostname: ResourceName,
    pub port: Option<u16>,
    pub starttls: Option<bool>,
    pub username: ResourceName,
    pub secret: Secret,
    pub from_name: ResourceName,
    pub from_email: Email,
}

impl Sanitize for JsonSmtp {
    fn sanitize(&mut self) {
        self.secret.sanitize();
    }
}
