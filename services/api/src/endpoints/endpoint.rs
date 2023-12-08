use std::fmt;

use dropshot::{
    HttpResponseAccepted, HttpResponseCreated, HttpResponseDeleted, HttpResponseHeaders,
    HttpResponseOk,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::util::headers::CorsHeaders;

pub type CorsResponse = HttpResponseHeaders<HttpResponseOk<String>, CorsHeaders>;
pub type ResponseOk<T> = HttpResponseHeaders<HttpResponseOk<T>, CorsHeaders>;
pub type ResponseCreated<T> = HttpResponseHeaders<HttpResponseCreated<T>, CorsHeaders>;
pub type ResponseAccepted<T> = HttpResponseHeaders<HttpResponseAccepted<T>, CorsHeaders>;
pub type ResponseDeleted = HttpResponseHeaders<HttpResponseDeleted, CorsHeaders>;

#[derive(Copy, Clone)]
pub enum Endpoint {
    Get(Get),
    Post(Post),
    Put(Put),
    Patch(Patch),
    Delete(Delete),
}

impl Endpoint {
    pub fn cors(endpoints: &[Self]) -> CorsResponse {
        HttpResponseHeaders::new(HttpResponseOk(String::new()), CorsHeaders::new(endpoints))
    }
}

impl From<Get> for Endpoint {
    fn from(method: Get) -> Self {
        Endpoint::Get(method)
    }
}
impl From<Post> for Endpoint {
    fn from(method: Post) -> Self {
        Endpoint::Post(method)
    }
}
impl From<Put> for Endpoint {
    fn from(method: Put) -> Self {
        Endpoint::Put(method)
    }
}
impl From<Patch> for Endpoint {
    fn from(method: Patch) -> Self {
        Endpoint::Patch(method)
    }
}
impl From<Delete> for Endpoint {
    fn from(method: Delete) -> Self {
        Endpoint::Delete(method)
    }
}

impl fmt::Display for Endpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Endpoint::Get(method) => method.to_string(),
                Endpoint::Post(method) => method.to_string(),
                Endpoint::Put(method) => method.to_string(),
                Endpoint::Patch(method) => method.to_string(),
                Endpoint::Delete(method) => method.to_string(),
            }
        )
    }
}

#[derive(Copy, Clone)]
pub struct Get;

impl From<Get> for http::Method {
    fn from(_: Get) -> Self {
        http::Method::GET
    }
}

impl fmt::Display for Get {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{method}", method = http::Method::from(*self))
    }
}

impl Get {
    pub fn response_ok<T>(body: T, auth: bool) -> ResponseOk<T>
    where
        T: JsonSchema + Serialize + Send + Sync,
    {
        if auth {
            Self::auth_response_ok(body)
        } else {
            Self::pub_response_ok(body)
        }
    }

    pub fn pub_response_ok<T>(body: T) -> ResponseOk<T>
    where
        T: JsonSchema + Serialize + Send + Sync,
    {
        let headers = CorsHeaders::new_pub(&http::Method::from(Self));
        Self::response_ok_inner(body, headers)
    }

    pub fn auth_response_ok<T>(body: T) -> ResponseOk<T>
    where
        T: JsonSchema + Serialize + Send + Sync,
    {
        let headers = CorsHeaders::new_auth(&http::Method::from(Self));
        Self::response_ok_inner(body, headers)
    }

    fn response_ok_inner<T, H>(body: T, headers: H) -> ResponseOk<T>
    where
        T: JsonSchema + Serialize + Send + Sync,
        H: Into<CorsHeaders>,
    {
        HttpResponseHeaders::new(HttpResponseOk(body), headers.into())
    }
}

macro_rules! impl_response_accepted {
    ($method:ident, $http:ident) => {
        impl From<$method> for http::Method {
            fn from(_: $method) -> Self {
                http::Method::$http
            }
        }

        impl fmt::Display for $method {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{method}", method = http::Method::from(*self))
            }
        }

        impl $method {
            pub fn response_accepted<T>(body: T, auth: bool) -> ResponseAccepted<T>
            where
                T: JsonSchema + Serialize + Send + Sync,
            {
                if auth {
                    Self::auth_response_accepted(body)
                } else {
                    Self::pub_response_accepted(body)
                }
            }

            pub fn pub_response_accepted<T>(body: T) -> ResponseAccepted<T>
            where
                T: JsonSchema + Serialize + Send + Sync,
            {
                let headers = CorsHeaders::new_pub(&http::Method::from(Self));
                Self::response_accepted_inner(body, headers)
            }

            pub fn auth_response_accepted<T>(body: T) -> ResponseAccepted<T>
            where
                T: JsonSchema + Serialize + Send + Sync,
            {
                let headers = CorsHeaders::new_auth(&http::Method::from(Self));
                Self::response_accepted_inner(body, headers)
            }

            fn response_accepted_inner<T, H>(body: T, headers: H) -> ResponseAccepted<T>
            where
                T: JsonSchema + Serialize + Send + Sync,
                H: Into<CorsHeaders>,
            {
                HttpResponseHeaders::new(HttpResponseAccepted(body), headers.into())
            }
        }
    };
}

#[derive(Copy, Clone)]
pub struct Post;
impl_response_accepted!(Post, POST);

impl Post {
    pub fn response_created<T>(body: T, auth: bool) -> ResponseCreated<T>
    where
        T: JsonSchema + Serialize + Send + Sync,
    {
        if auth {
            Self::auth_response_created(body)
        } else {
            Self::pub_response_created(body)
        }
    }

    pub fn pub_response_created<T>(body: T) -> ResponseCreated<T>
    where
        T: JsonSchema + Serialize + Send + Sync,
    {
        let headers = CorsHeaders::new_pub(&http::Method::from(Self));
        Self::response_created_inner(body, headers)
    }

    pub fn auth_response_created<T>(body: T) -> ResponseCreated<T>
    where
        T: JsonSchema + Serialize + Send + Sync,
    {
        let headers = CorsHeaders::new_auth(&http::Method::from(Self));
        Self::response_created_inner(body, headers)
    }

    fn response_created_inner<T, H>(body: T, headers: H) -> ResponseCreated<T>
    where
        T: JsonSchema + Serialize + Send + Sync,
        H: Into<CorsHeaders>,
    {
        HttpResponseHeaders::new(HttpResponseCreated(body), headers.into())
    }
}

#[derive(Copy, Clone)]
pub struct Put;
impl_response_accepted!(Put, PUT);

#[derive(Copy, Clone)]
pub struct Patch;
impl_response_accepted!(Patch, PATCH);

#[derive(Copy, Clone)]
pub struct Delete;

impl From<Delete> for http::Method {
    fn from(_: Delete) -> Self {
        http::Method::DELETE
    }
}

impl fmt::Display for Delete {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{method}", method = http::Method::from(*self))
    }
}

impl Delete {
    pub fn response_deleted<T>(auth: bool) -> ResponseDeleted {
        if auth {
            Self::auth_response_deleted()
        } else {
            Self::pub_response_deleted()
        }
    }

    pub fn pub_response_deleted() -> ResponseDeleted {
        let headers = CorsHeaders::new_pub(&http::Method::from(Self));
        Self::response_deleted_inner(headers)
    }

    pub fn auth_response_deleted() -> ResponseDeleted {
        let headers = CorsHeaders::new_auth(&http::Method::from(Self));
        Self::response_deleted_inner(headers)
    }

    fn response_deleted_inner<H>(headers: H) -> ResponseDeleted
    where
        H: Into<CorsHeaders>,
    {
        HttpResponseHeaders::new(HttpResponseDeleted(), headers.into())
    }
}
