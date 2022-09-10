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

use endpoints::auth::Resource as AuthResource;
use endpoints::users::Resource as UsersResource;

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

#[derive(Debug, Clone, Copy)]
pub enum Endpoint {
    Auth(AuthResource),
    Users(UsersResource),
    Orgs(OrgsResource),
    Ping(PingMethod),
}

impl From<UsersResource> for Endpoint {
    fn from(users: UsersResource) -> Self {
        Self::Users(users)
    }
}

impl WordStr for Endpoint {
    fn singular(&self) -> &str {
        match self {
            Self::Auth(_) => todo!(),
            Self::Users(users) => users.singular(),
            Self::Orgs(_) => todo!(),
            Self::Ping(_) => todo!(),
        }
    }

    fn plural(&self) -> &str {
        match self {
            Self::Auth(_) => todo!(),
            Self::Users(users) => users.plural(),
            Self::Orgs(_) => todo!(),
            Self::Ping(_) => todo!(),
        }
    }
}

#[derive(Debug, Display, Clone, Copy)]
pub enum OrgsResource {
    Benchmark,
    Branch,
    Perf,
    Ping,
    Project,
    Report,
    Testbed,
    Threshold,
}
