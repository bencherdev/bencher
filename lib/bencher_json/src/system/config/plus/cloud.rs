use std::collections::HashMap;

use bencher_valid::{Sanitize, Secret};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonCloud {
    pub billing: JsonBilling,
    pub license_pem: Secret,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sentry: Option<Secret>,
}

impl Sanitize for JsonCloud {
    fn sanitize(&mut self) {
        self.billing.sanitize();
        self.license_pem.sanitize();
        self.sentry.sanitize();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonBilling {
    pub secret_key: Secret,
    pub products: JsonProducts,
}

impl Sanitize for JsonBilling {
    fn sanitize(&mut self) {
        self.secret_key.sanitize();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonProducts {
    pub team: JsonProduct,
    pub enterprise: JsonProduct,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonProduct {
    pub id: String,
    pub metered: HashMap<String, String>,
    pub licensed: HashMap<String, String>,
}
