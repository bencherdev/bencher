use bencher_valid::{NonEmpty, Sanitize, Secret};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonIndex {
    pub google: Option<JsonIndexGoogle>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonIndexGoogle {
    pub private_key: Secret,
    pub client_email: NonEmpty,
    pub token_uri: NonEmpty,
}

impl Sanitize for JsonIndex {
    fn sanitize(&mut self) {
        self.google.sanitize();
    }
}

impl Sanitize for JsonIndexGoogle {
    fn sanitize(&mut self) {
        self.private_key.sanitize();
    }
}
