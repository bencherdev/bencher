use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::TotalCount;

const ALL_ORIGIN: &str = "*";
const PUB_ALLOW_HEADERS: &str = "Content-Type";
const AUTH_ALLOW_HEADERS: &str = "Content-Type, Authorization";
const EXPOSE_HEADERS: &str = "X-Total-Count";

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub struct CorsHeaders {
    pub access_control_allow_origin: String,
    pub access_control_allow_methods: String,
    pub access_control_allow_headers: String,
    pub access_control_expose_headers: String,
    pub x_total_count: String,
}

impl CorsHeaders {
    pub fn new<T>(methods: &[T]) -> Self
    where
        T: ToString,
    {
        let methods = methods_str(methods);
        Self::new_origin_all(methods, AUTH_ALLOW_HEADERS.to_owned(), None)
    }

    pub fn new_with_total_count<T>(methods: &[T], total_count: TotalCount) -> Self
    where
        T: ToString,
    {
        let methods = methods_str(methods);
        Self::new_origin_all(methods, AUTH_ALLOW_HEADERS.to_owned(), Some(total_count))
    }

    pub fn new_pub<T>(methods: &T) -> Self
    where
        T: ToString,
    {
        Self::new_origin_all(methods.to_string(), PUB_ALLOW_HEADERS.to_owned(), None)
    }

    pub fn new_pub_with_total_count<T>(methods: &T, total_count: TotalCount) -> Self
    where
        T: ToString,
    {
        Self::new_origin_all(
            methods.to_string(),
            PUB_ALLOW_HEADERS.to_owned(),
            Some(total_count),
        )
    }

    pub fn new_auth<T>(methods: &T) -> Self
    where
        T: ToString,
    {
        Self::new_origin_all(methods.to_string(), AUTH_ALLOW_HEADERS.to_owned(), None)
    }

    pub fn new_auth_with_total_count<T>(methods: &T, total_count: TotalCount) -> Self
    where
        T: ToString,
    {
        Self::new_origin_all(
            methods.to_string(),
            AUTH_ALLOW_HEADERS.to_owned(),
            Some(total_count),
        )
    }

    fn new_origin_all(methods: String, headers: String, total_count: Option<TotalCount>) -> Self {
        CorsHeaders {
            access_control_allow_origin: ALL_ORIGIN.to_owned(),
            access_control_allow_methods: methods,
            access_control_allow_headers: headers,
            access_control_expose_headers: EXPOSE_HEADERS.to_owned(),
            x_total_count: total_count.unwrap_or(TotalCount::ONE).to_string(),
        }
    }
}

fn methods_str<T>(methods: &[T]) -> String
where
    T: ToString,
{
    methods
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<String>>()
        .join(", ")
}
