use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::IntoEndpoint;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CorsHeaders {
    #[serde(rename = "Access-Control-Allow-Origin")]
    pub access_control_allow_origin: String,
    #[serde(rename = "Access-Control-Allow-Methods")]
    pub access_control_allow_methods: String,
    #[serde(rename = "Access-Control-Allow-Headers")]
    pub access_control_allow_headers: String,
    #[serde(rename = "Access-Control-Allow-Credentials")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_control_allow_credentials: Option<bool>,
}

impl CorsHeaders {
    pub fn new_origin_all(methods: String, headers: String, credentials: Option<bool>) -> Self {
        CorsHeaders {
            access_control_allow_origin: "*".into(),
            access_control_allow_methods: methods,
            access_control_allow_headers: headers,
            access_control_allow_credentials: credentials,
        }
    }

    pub fn new_pub(methods: String) -> Self {
        Self::new_origin_all(methods, "Content-Type".into(), None)
    }

    pub fn new_pub_endpoint(endpoint: impl IntoEndpoint) -> Self {
        Self::new_origin_all(
            endpoint.into_endpoint().to_string(),
            "Content-Type".into(),
            None,
        )
    }

    pub fn new_auth(methods: String) -> Self {
        Self::new_origin_all(methods, "Content-Type, Authorization".into(), None)
    }
}
