use std::fmt;

use dropshot::{
    HttpResponseAccepted, HttpResponseCreated, HttpResponseDeleted, HttpResponseHeaders,
    HttpResponseOk,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::util::headers::{CorsHeaders, CorsLsHeaders, TotalCount};

pub type CorsResponse = HttpResponseHeaders<HttpResponseOk<()>, CorsHeaders>;
pub type CorsLsResponse = HttpResponseHeaders<HttpResponseOk<()>, CorsLsHeaders>;
pub type ResponseOk<T> = HttpResponseHeaders<HttpResponseOk<T>, CorsHeaders>;
pub type ResponseOkLs<T> = HttpResponseHeaders<HttpResponseOk<T>, CorsLsHeaders>;
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
        HttpResponseHeaders::new(HttpResponseOk(()), CorsHeaders::new(endpoints))
    }

    pub fn cors_ls(endpoints: &[Self]) -> CorsLsResponse {
        HttpResponseHeaders::new(HttpResponseOk(()), CorsLsHeaders::new(endpoints))
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

macro_rules! impl_method {
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
    };
}

macro_rules! impl_response {
    ($method:ident, $status:ident, $response:ident) => {
        paste::paste! {
            impl $method {
                pub fn [<response_ $status>]<T>(body: T, auth: bool) -> $response<T>
                where
                    T: JsonSchema + Serialize + Send + Sync,
                {
                    if auth {
                        Self::[<auth_response_ $status>](body)
                    } else {
                        Self::[<pub_response_ $status>](body)
                    }
                }

                pub fn [<pub_response_ $status>]<T>(body: T) -> $response<T>
                where
                    T: JsonSchema + Serialize + Send + Sync,
                {
                    let headers = CorsHeaders::new_pub(&http::Method::from(Self));
                    Self::[<response_ $status _inner>](body, headers)
                }

                pub fn [<auth_response_ $status>]<T>(body: T) -> $response<T>
                where
                    T: JsonSchema + Serialize + Send + Sync,
                {
                    let headers = CorsHeaders::new_auth(&http::Method::from(Self));
                    Self::[<response_ $status _inner>](body, headers)
                }

                fn [<response_ $status _inner>]<T, H>(body: T, headers: H) -> $response<T>
                where
                    T: JsonSchema + Serialize + Send + Sync,
                    H: Into<CorsHeaders>,
                {
                    HttpResponseHeaders::new([<Http $response>](body), headers.into())
                }
            }
        }
    };
}

macro_rules! impl_response_ok {
    ($method:ident) => {
        impl_response!($method, ok, ResponseOk);
    };
}

macro_rules! impl_response_created {
    ($method:ident) => {
        impl_response!($method, created, ResponseCreated);
    };
}

macro_rules! impl_response_accepted {
    ($method:ident) => {
        impl_response!($method, accepted, ResponseAccepted);
    };
}

#[derive(Copy, Clone)]
pub struct Get;
impl_method!(Get, GET);
impl_response_ok!(Get);

impl Get {
    pub fn response_ok_ls<T>(body: T, auth: bool, total_count: TotalCount) -> ResponseOkLs<T>
    where
        T: JsonSchema + Serialize + Send + Sync,
    {
        if auth {
            Self::auth_response_ok_ls(body, total_count)
        } else {
            Self::pub_response_ok_ls(body, total_count)
        }
    }

    pub fn pub_response_ok_ls<T>(body: T, total_count: TotalCount) -> ResponseOkLs<T>
    where
        T: JsonSchema + Serialize + Send + Sync,
    {
        let headers = CorsLsHeaders::new_pub(&http::Method::from(Self), total_count);
        Self::response_ok_ls_inner(body, headers)
    }

    pub fn auth_response_ok_ls<T>(body: T, total_count: TotalCount) -> ResponseOkLs<T>
    where
        T: JsonSchema + Serialize + Send + Sync,
    {
        let headers = CorsLsHeaders::new_auth(&http::Method::from(Self), total_count);
        Self::response_ok_ls_inner(body, headers)
    }

    fn response_ok_ls_inner<T, H>(body: T, headers: H) -> ResponseOkLs<T>
    where
        T: JsonSchema + Serialize + Send + Sync,
        H: Into<CorsLsHeaders>,
    {
        HttpResponseHeaders::new(HttpResponseOk(body), headers.into())
    }
}

#[derive(Copy, Clone)]
pub struct Post;
impl_method!(Post, POST);
impl_response_ok!(Post);
impl_response_created!(Post);
impl_response_accepted!(Post);

#[derive(Copy, Clone)]
pub struct Put;
impl_method!(Put, PUT);
impl_response_ok!(Put);
impl_response_accepted!(Put);

#[derive(Copy, Clone)]
pub struct Patch;
impl_method!(Patch, PATCH);
impl_response_ok!(Patch);

#[derive(Copy, Clone)]
pub struct Delete;
impl_method!(Delete, DELETE);

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
