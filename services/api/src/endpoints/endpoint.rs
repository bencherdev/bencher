use dropshot::{HttpCodedResponse, HttpResponseAccepted, HttpResponseHeaders, HttpResponseOk};
use schemars::JsonSchema;
use serde::Serialize;

use crate::{util::headers::CorsHeaders, ApiError};

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

    pub fn err(&self, e: ApiError) -> ApiError {
        let api_error: ApiError = self.into();
        tracing::info!("{api_error}: {e}");
        api_error
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

impl From<&Endpoint> for ApiError {
    fn from(endpoint: &Endpoint) -> Self {
        match endpoint.method {
            Method::GetOne => ApiError::GetOne(endpoint.resource),
            Method::GetLs => ApiError::GetLs(endpoint.resource),
            Method::Post => ApiError::Post(endpoint.resource),
            Method::Put => ApiError::Put(endpoint.resource),
            Method::Delete => ApiError::Delete(endpoint.resource),
        }
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
