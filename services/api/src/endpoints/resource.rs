use crate::WordStr;

use super::auth::Resource as AuthResource;
use super::orgs::Resource as OrgsResource;
use super::users::Resource as UsersResource;

#[derive(Debug, Clone, Copy)]
pub enum Resource {
    Auth(AuthResource),
    Users(UsersResource),
    Orgs(OrgsResource),
    Ping,
}

impl From<AuthResource> for Resource {
    fn from(resource: AuthResource) -> Self {
        Self::Auth(resource)
    }
}

impl From<UsersResource> for Resource {
    fn from(resource: UsersResource) -> Self {
        Self::Users(resource)
    }
}

impl From<OrgsResource> for Resource {
    fn from(resource: OrgsResource) -> Self {
        Self::Orgs(resource)
    }
}

impl WordStr for Resource {
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
