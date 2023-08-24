use std::fmt;

use dropshot::{HttpCodedResponse, HttpResponseAccepted, HttpResponseHeaders, HttpResponseOk};
use schemars::JsonSchema;
use serde::Serialize;

use crate::{util::headers::CorsHeaders, ApiError, WordStr};

use super::{Method, Resource};

pub type ResponseOk<T> = HttpResponseHeaders<HttpResponseOk<T>, CorsHeaders>;
pub type ResponseAccepted<T> = HttpResponseHeaders<HttpResponseAccepted<T>, CorsHeaders>;

#[derive(Copy, Clone)]
pub struct Endpoint {
    resource: Resource,
    method: Method,
}

impl Endpoint {
    pub fn new(resource: impl Into<Resource>, method: Method) -> Self {
        Self {
            resource: resource.into(),
            method,
        }
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn err(&self, _e: ApiError) -> ApiError {
        // tracing::info!("{api_error}: {e}");
        ApiError::Endpoint(*self)
    }

    pub fn pub_response_headers<R, T>(&self, body: R) -> HttpResponseHeaders<R, CorsHeaders>
    where
        R: HttpCodedResponse<Body = T>,
        T: JsonSchema + Serialize + Send + Sync,
    {
        HttpResponseHeaders::new(body, self.pub_header())
    }

    pub fn response_headers<R, T>(&self, body: R) -> HttpResponseHeaders<R, CorsHeaders>
    where
        R: HttpCodedResponse<Body = T>,
        T: JsonSchema + Serialize + Send + Sync,
    {
        HttpResponseHeaders::new(body, self.header())
    }

    pub fn pub_header(&self) -> CorsHeaders {
        CorsHeaders::new_origin_all(
            http::Method::from(self.method).to_string(),
            "Content-Type".into(),
            None,
        )
    }

    pub fn header(&self) -> CorsHeaders {
        CorsHeaders::new_origin_all(
            http::Method::from(self.method).to_string(),
            "Content-Type, Authorization".into(),
            None,
        )
    }
}

impl fmt::Debug for Endpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        <Endpoint as fmt::Display>::fmt(self, f)
    }
}

impl fmt::Display for Endpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let method = http::Method::from(self.method);
        let resource = match self.method {
            Method::GetOne | Method::Post | Method::Put | Method::Patch | Method::Delete => {
                self.resource.singular()
            },
            Method::GetLs => self.resource.plural(),
        };
        write!(f, "{method} {resource}")
    }
}

macro_rules! pub_response_ok {
    ($endpoint:expr, $body:expr) => {
        Ok($endpoint.pub_response_headers(dropshot::HttpResponseOk($body)))
    };
}

pub(crate) use pub_response_ok;

macro_rules! pub_response_accepted {
    ($endpoint:expr, $body:expr) => {
        Ok($endpoint.pub_response_headers(dropshot::HttpResponseAccepted($body)))
    };
}

pub(crate) use pub_response_accepted;

macro_rules! response_ok {
    ($endpoint:expr, $body:expr) => {
        Ok($endpoint.response_headers(dropshot::HttpResponseOk($body)))
    };
}

pub(crate) use response_ok;

macro_rules! response_accepted {
    ($endpoint:expr, $body:expr) => {
        Ok($endpoint.response_headers(dropshot::HttpResponseAccepted($body)))
    };
}

pub(crate) use response_accepted;
