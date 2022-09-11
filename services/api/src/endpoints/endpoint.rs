use dropshot::{HttpCodedResponse, HttpResponseHeaders, HttpResponseOk};
use schemars::JsonSchema;
use serde::Serialize;

use crate::{util::headers::CorsHeaders, ApiError};

use super::{Method, Resource};

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
        tracing::error!("{api_error}: {e}");
        api_error
    }

    pub fn pub_response_headers<T>(
        &self,
        body: T,
    ) -> HttpResponseHeaders<HttpResponseOk<T>, CorsHeaders>
    where
        T: JsonSchema + Serialize + Send + Sync,
    {
        HttpResponseHeaders::new(HttpResponseOk(body), self.pub_header())
    }

    pub fn pub_header(&self) -> CorsHeaders {
        CorsHeaders::new_origin_all(
            http::Method::from(self.method).to_string(),
            "Content-Type".into(),
            None,
        )
    }

    pub fn response_headers<R, T>(&self, body: R) -> HttpResponseHeaders<R, CorsHeaders>
    where
        R: HttpCodedResponse<Body = T>,
        T: JsonSchema + Serialize + Send + Sync,
    {
        HttpResponseHeaders::new(body, self.header())
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
