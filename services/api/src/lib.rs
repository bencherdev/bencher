#[macro_use]
extern crate diesel;

use std::fmt;

pub mod endpoints;
pub mod error;
pub mod model;
pub mod schema;
pub mod util;

pub use error::ApiError;

pub trait ToMethod {
    fn to_method(&self) -> http::Method;
}

pub trait IntoEndpoint {
    fn into_endpoint(self) -> Endpoint;
}

#[derive(Debug, Clone, Copy)]
pub enum Endpoint {
    Auth(AuthEndpoint),
    User(UserEndpoint),
    Org(OrgEndpoint),
    Ping(PingMethod),
}

#[derive(Debug, Clone, Copy)]
pub enum PingMethod {
    Get,
}

impl IntoEndpoint for PingMethod {
    fn into_endpoint(self) -> Endpoint {
        Endpoint::Ping(self)
    }
}

impl ToMethod for PingMethod {
    fn to_method(&self) -> http::Method {
        match self {
            Self::Get => http::Method::GET,
        }
    }
}

impl fmt::Display for PingMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_method())
    }
}

#[derive(Debug, Clone, Copy)]
pub enum AuthEndpoint {
    Confirm,
    Invite,
    Login,
    Signup,
}

#[derive(Debug, Clone, Copy)]
pub enum UserEndpoint {
    Token,
}

#[derive(Debug, Clone, Copy)]
pub enum OrgEndpoint {
    Benchmark,
    Branch,
    Perf,
    Ping,
    Project,
    Report,
    Testbed,
    Threshold,
}
