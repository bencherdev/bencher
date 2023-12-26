use bencher_valid::{NonEmpty, Sanitize, Secret};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonGitHub {
    pub client_id: NonEmpty,
    pub client_secret: Secret,
}

impl Sanitize for JsonGitHub {
    fn sanitize(&mut self) {
        self.client_secret.sanitize();
    }
}
