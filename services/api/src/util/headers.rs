use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
            access_control_allow_origin: "*".to_owned(),
            access_control_allow_methods: methods,
            access_control_allow_headers: headers,
            access_control_allow_credentials: credentials,
        }
    }

    pub fn new<T>(methods: &[T]) -> Self
    where
        T: ToString,
    {
        let methods = methods
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<String>>()
            .join(", ");

        Self::new_origin_all(methods, "*".to_owned(), None)
    }

    pub fn new_pub<T>(methods: &T) -> Self
    where
        T: ToString,
    {
        Self::new_origin_all(methods.to_string(), "Content-Type".to_owned(), None)
    }

    pub fn new_auth<T>(methods: &T) -> Self
    where
        T: ToString,
    {
        Self::new_origin_all(
            methods.to_string(),
            "Content-Type, Authorization".to_owned(),
            None,
        )
    }
}
