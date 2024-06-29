use std::fmt;

use dropshot::HttpError;
use http::StatusCode;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::issue_error;

const ALL_ORIGIN: &str = "*";
const ALL_HEADERS: &str = "*";
const PUB_HEADERS: &str = "Content-Type";
const AUTH_HEADERS: &str = "Content-Type, Authorization";
const EXPOSE_HEADERS: &str = "X-Total-Count";

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub struct CorsHeaders {
    pub access_control_allow_origin: String,
    pub access_control_allow_methods: String,
    pub access_control_allow_headers: String,
}

impl CorsHeaders {
    pub fn new<T>(methods: &[T]) -> Self
    where
        T: ToString,
    {
        let methods = methods_str(methods);
        Self::new_origin_all(methods, ALL_HEADERS.to_owned())
    }

    pub fn new_pub<T>(methods: &T) -> Self
    where
        T: ToString,
    {
        Self::new_origin_all(methods.to_string(), PUB_HEADERS.to_owned())
    }

    pub fn new_auth<T>(methods: &T) -> Self
    where
        T: ToString,
    {
        Self::new_origin_all(methods.to_string(), AUTH_HEADERS.to_owned())
    }

    fn new_origin_all(methods: String, headers: String) -> Self {
        CorsHeaders {
            access_control_allow_origin: ALL_ORIGIN.to_owned(),
            access_control_allow_methods: methods,
            access_control_allow_headers: headers,
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

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub struct CorsLsHeaders {
    pub access_control_allow_origin: String,
    pub access_control_allow_methods: String,
    pub access_control_allow_headers: String,
    pub access_control_expose_headers: String,
    pub x_total_count: String,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema)]
pub struct TotalCount(u32);

impl TryFrom<i64> for TotalCount {
    type Error = HttpError;

    fn try_from(total_count: i64) -> Result<Self, Self::Error> {
        match u32::try_from(total_count) {
            Ok(total_count) => Ok(TotalCount(total_count)),
            Err(err) => Err(issue_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to count resource total.",
                &format!("Failed to count resource total: {total_count}"),
                err,
            )),
        }
    }
}

impl fmt::Display for TotalCount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 == 0 {
            write!(f, "")
        } else {
            write!(f, "{}", self.0)
        }
    }
}

impl CorsLsHeaders {
    pub fn new<T>(methods: &[T]) -> Self
    where
        T: ToString,
    {
        let methods = methods_str(methods);
        Self::new_origin_all(methods, ALL_HEADERS.to_owned(), None)
    }

    pub fn new_pub<T>(methods: &T, total_count: TotalCount) -> Self
    where
        T: ToString,
    {
        Self::new_origin_all(
            methods.to_string(),
            PUB_HEADERS.to_owned(),
            Some(total_count),
        )
    }

    pub fn new_auth<T>(methods: &T, total_count: TotalCount) -> Self
    where
        T: ToString,
    {
        Self::new_origin_all(
            methods.to_string(),
            AUTH_HEADERS.to_owned(),
            Some(total_count),
        )
    }

    fn new_origin_all(methods: String, headers: String, total_count: Option<TotalCount>) -> Self {
        CorsLsHeaders {
            access_control_allow_origin: ALL_ORIGIN.to_owned(),
            access_control_allow_methods: methods,
            access_control_allow_headers: headers,
            access_control_expose_headers: EXPOSE_HEADERS.to_owned(),
            x_total_count: total_count.unwrap_or_default().to_string(),
        }
    }
}
