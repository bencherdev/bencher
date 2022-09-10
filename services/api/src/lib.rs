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
use endpoints::users::Endpoint as UsersEndpoint;

pub trait ToMethod {
    fn to_method(&self) -> http::Method;
}

pub trait WordStr {
    fn singular(&self) -> &str;
    fn plural(&self) -> &str;
}

pub trait IntoEndpoint {
    fn into_endpoint(self) -> Endpoint;
}

#[derive(Debug, Display, Clone, Copy)]
pub enum Endpoint {
    Auth(AuthEndpoint),
    Users(UsersEndpoint),
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
