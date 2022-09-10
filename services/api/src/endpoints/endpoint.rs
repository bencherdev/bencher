use crate::WordStr;

use super::auth::Resource as AuthResource;
use super::users::Resource as UsersResource;
use super::users::Resource as OrgsResource;

#[derive(Debug, Clone, Copy)]
pub enum Endpoint {
    Auth(AuthResource),
    Users(UsersResource),
    Orgs(OrgsResource),
    Ping,
}

impl From<UsersResource> for Endpoint {
    fn from(users: UsersResource) -> Self {
        Self::Users(users)
    }
}

impl WordStr for Endpoint {
    fn singular(&self) -> &str {
        match self {
            Self::Auth(auth) => auth.singular(),
            Self::Users(users) => users.singular(),
            Self::Orgs(orgs) => orgs.singular(),
            Self::Ping => "ping",
        }
    }

    fn plural(&self) -> &str {
        match self {
            Self::Auth(auth) => auth.plural(),
            Self::Users(users) => users.plural(),
            Self::Orgs(orgs) => orgs.plural(),
            Self::Ping => "pings",
        }
    }
}
