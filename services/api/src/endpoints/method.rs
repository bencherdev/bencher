#[derive(Copy, Clone)]
pub enum Method {
    GetOne,
    GetLs,
    Post,
    Put,
    Patch,
    Delete,
}

impl From<Method> for http::Method {
    fn from(method: Method) -> Self {
        match method {
            Method::GetOne | Method::GetLs => http::Method::GET,
            Method::Post => http::Method::POST,
            Method::Put => http::Method::PUT,
            Method::Patch => http::Method::PATCH,
            Method::Delete => http::Method::DELETE,
        }
    }
}
