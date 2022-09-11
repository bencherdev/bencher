use dropshot::HttpCodedResponse;
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Copy, Clone)]
pub enum Method {
    GetOne,
    GetLs,
    Post,
    Put,
    Delete,
}

impl From<Method> for http::Method {
    fn from(method: Method) -> Self {
        match method {
            Method::GetOne | Method::GetLs => http::Method::GET,
            Method::Post => http::Method::POST,
            Method::Put => http::Method::PUT,
            Method::Delete => http::Method::DELETE,
        }
    }
}
