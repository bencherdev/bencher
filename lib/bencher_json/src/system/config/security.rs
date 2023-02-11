use bencher_valid::{Sanitize, Secret};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonSecurity {
    pub issuer: Option<String>,
    pub secret_key: Secret,
}

impl Sanitize for JsonSecurity {
    fn sanitize(&mut self) {
        self.secret_key.sanitize();
    }
}
