use bencher_valid::{NonEmpty, Sanitize, Secret, Url};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonIndex {
    pub bing: JsonBingIndex,
    pub google: JsonGoogleIndex,
}
impl Sanitize for JsonIndex {
    fn sanitize(&mut self) {
        self.google.sanitize();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonBingIndex {
    pub key: NonEmpty,
    pub key_location: Option<Url>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonGoogleIndex {
    pub private_key: Secret,
    pub client_email: NonEmpty,
    pub token_uri: NonEmpty,
}

impl Sanitize for JsonGoogleIndex {
    fn sanitize(&mut self) {
        self.private_key.sanitize();
    }
}
