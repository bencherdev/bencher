#[macro_use]
extern crate diesel;

use derive_more::Display;
use endpoints::ping::PingMethod;

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

#[derive(Debug, Display, Clone, Copy)]
pub enum Endpoint {
    Auth(AuthEndpoint),
    User(UserEndpoint),
    Org(OrgEndpoint),
    Ping(PingMethod),
}

#[derive(Debug, Display, Clone, Copy)]
pub enum AuthEndpoint {
    Confirm,
    Invite,
    Login,
    Signup,
}

#[derive(Debug, Display, Clone, Copy)]
pub enum UserEndpoint {
    Token,
}

#[derive(Debug, Display, Clone, Copy)]
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
