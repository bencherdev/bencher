use std::fmt;

use dropshot::{HttpCodedResponse, HttpResponseAccepted, HttpResponseHeaders, HttpResponseOk};
use schemars::JsonSchema;
use serde::Serialize;

use crate::{util::headers::CorsHeaders, ApiError};

pub type CorsResponse = HttpResponseHeaders<HttpResponseOk<String>, CorsHeaders>;
pub type ResponseOk<T> = HttpResponseHeaders<HttpResponseOk<T>, CorsHeaders>;
pub type ResponseAccepted<T> = HttpResponseHeaders<HttpResponseAccepted<T>, CorsHeaders>;

#[derive(Copy, Clone)]
pub enum Endpoint {
    GetOnePub,
    GetOne,
    GetLsPub,
    GetLs,
    Post,
    Put,
    Patch,
    Delete,
}

#[derive(Copy, Clone)]
pub struct Get;

impl From<Get> for http::Method {
    fn from(_: Get) -> Self {
        http::Method::GET
    }
}

impl Get {
    pub fn response_ok<T>(
        body: T,
        auth: bool,
    ) -> HttpResponseHeaders<HttpResponseOk<T>, CorsHeaders>
    where
        T: JsonSchema + Serialize + Send + Sync,
    {
        if auth {
            Self::auth_response_ok(body)
        } else {
            Self::pub_response_ok(body)
        }
    }

    pub fn pub_response_ok<T>(body: T) -> HttpResponseHeaders<HttpResponseOk<T>, CorsHeaders>
    where
        T: JsonSchema + Serialize + Send + Sync,
    {
        let headers = CorsHeaders::new_pub(&http::Method::from(Self));
        response_ok_inner(body, headers)
    }

    pub fn auth_response_ok<T>(body: T) -> HttpResponseHeaders<HttpResponseOk<T>, CorsHeaders>
    where
        T: JsonSchema + Serialize + Send + Sync,
    {
        let headers = CorsHeaders::new_auth(&http::Method::from(Self));
        response_ok_inner(body, headers)
    }
}

macro_rules! impl_response_accepted {
    ($method:ident, $http:ident) => {
        impl From<$method> for http::Method {
            fn from(_: $method) -> Self {
                http::Method::$http
            }
        }

        impl $method {
            pub fn response_accepted<T>(
                body: T,
                auth: bool,
            ) -> HttpResponseHeaders<HttpResponseAccepted<T>, CorsHeaders>
            where
                T: JsonSchema + Serialize + Send + Sync,
            {
                if auth {
                    Self::auth_response_accepted(body)
                } else {
                    Self::pub_response_accepted(body)
                }
            }

            pub fn pub_response_accepted<T>(
                body: T,
            ) -> HttpResponseHeaders<HttpResponseAccepted<T>, CorsHeaders>
            where
                T: JsonSchema + Serialize + Send + Sync,
            {
                let headers = CorsHeaders::new_pub(&http::Method::from(Self));
                response_accepted_inner(body, headers)
            }

            pub fn auth_response_accepted<T>(
                body: T,
            ) -> HttpResponseHeaders<HttpResponseAccepted<T>, CorsHeaders>
            where
                T: JsonSchema + Serialize + Send + Sync,
            {
                let headers = CorsHeaders::new_auth(&http::Method::from(Self));
                response_accepted_inner(body, headers)
            }
        }
    };
}

#[derive(Copy, Clone)]
pub struct Post;
impl_response_accepted!(Post, POST);

#[derive(Copy, Clone)]
pub struct Put;
impl_response_accepted!(Put, PUT);

#[derive(Copy, Clone)]
pub struct Patch;
impl_response_accepted!(Patch, PATCH);

#[derive(Copy, Clone)]
pub struct Delete;
impl_response_accepted!(Delete, DELETE);

fn response_ok_inner<T, H>(
    body: T,
    headers: H,
) -> HttpResponseHeaders<HttpResponseOk<T>, CorsHeaders>
where
    T: JsonSchema + Serialize + Send + Sync,
    H: Into<CorsHeaders>,
{
    HttpResponseHeaders::new(HttpResponseOk(body), headers.into())
}

pub fn response_accepted_inner<T, H>(
    body: T,
    headers: H,
) -> HttpResponseHeaders<HttpResponseAccepted<T>, CorsHeaders>
where
    T: JsonSchema + Serialize + Send + Sync,
    H: Into<CorsHeaders>,
{
    HttpResponseHeaders::new(HttpResponseAccepted(body), headers.into())
}

impl From<Endpoint> for http::Method {
    fn from(endpoint: Endpoint) -> Self {
        match endpoint {
            Endpoint::GetOnePub | Endpoint::GetOne | Endpoint::GetLsPub | Endpoint::GetLs => {
                http::Method::GET
            },
            Endpoint::Post => http::Method::POST,
            Endpoint::Put => http::Method::PUT,
            Endpoint::Patch => http::Method::PATCH,
            Endpoint::Delete => http::Method::DELETE,
        }
    }
}

impl Endpoint {
    pub fn cors(endpoints: &[Self]) -> CorsResponse {
        HttpResponseHeaders::new(
            HttpResponseOk(String::new()),
            CorsHeaders::new_cors(endpoints),
        )
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn err(self, _e: ApiError) -> ApiError {
        // tracing::info!("{api_error}: {e}");
        ApiError::Endpoint(self)
    }

    pub fn pub_response_headers<R, T>(self, body: R) -> HttpResponseHeaders<R, CorsHeaders>
    where
        R: HttpCodedResponse<Body = T>,
        T: JsonSchema + Serialize + Send + Sync,
    {
        HttpResponseHeaders::new(body, self.pub_header())
    }

    pub fn response_headers<R, T>(self, body: R) -> HttpResponseHeaders<R, CorsHeaders>
    where
        R: HttpCodedResponse<Body = T>,
        T: JsonSchema + Serialize + Send + Sync,
    {
        HttpResponseHeaders::new(body, self.header())
    }

    pub fn pub_header(self) -> CorsHeaders {
        CorsHeaders::new_pub(&self)
    }

    pub fn header(self) -> CorsHeaders {
        CorsHeaders::new_auth(&self)
    }
}

impl fmt::Debug for Endpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        <Endpoint as fmt::Display>::fmt(self, f)
    }
}

impl fmt::Display for Endpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{method}", method = http::Method::from(*self))
    }
}
