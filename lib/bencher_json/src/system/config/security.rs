use bencher_valid::{Sanitize, Secret};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonSecurity {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issuer: Option<String>,
    pub secret_key: Secret,
    /// Shared with the console's server, which relays the browser's requests to the API.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relay_secret: Option<Secret>,
}

impl Sanitize for JsonSecurity {
    fn sanitize(&mut self) {
        self.secret_key.sanitize();
        self.relay_secret.sanitize();
    }
}
