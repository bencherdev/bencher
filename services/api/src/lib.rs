#[macro_use]
extern crate diesel;

use derive_more::Display;
use endpoints::ping::Method as PingMethod;

pub mod endpoints;
pub mod error;
pub mod model;
pub mod schema;
pub mod util;

pub use error::ApiError;

use endpoints::auth::Endpoint as AuthEndpoint;
use endpoints::users::Resource as UsersResource;

pub trait ToMethod {
    fn to_method(&self) -> http::Method;
}

pub trait IntoEndpoint {
    fn into_endpoint(self) -> Endpoint;
}

#[derive(Debug, Display, Clone, Copy)]
pub enum Endpoint {
    Auth(AuthEndpoint),
    Users(UsersResource),
    Orgs(OrgsEndpoint),
    Ping(PingMethod),
}

#[derive(Debug, Display, Clone, Copy)]
pub enum OrgsEndpoint {
    Benchmark,
    Branch,
    Perf,
    Ping,
    Project,
    Report,
    Testbed,
    Threshold,
}
