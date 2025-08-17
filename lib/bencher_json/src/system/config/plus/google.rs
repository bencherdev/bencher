use bencher_valid::{NonEmpty, Sanitize, Secret, Url};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonGoogle {
    pub client_id: NonEmpty,
    pub client_secret: Secret,
    pub callback_url: Url,
}

impl Sanitize for JsonGoogle {
    fn sanitize(&mut self) {
        self.client_secret.sanitize();
    }
}
