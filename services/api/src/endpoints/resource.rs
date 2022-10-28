use crate::WordStr;

use super::organization::Resource as OrganizationResource;
use super::project::Resource as ProjectResource;
use super::system::{auth::Resource as AuthResource, server::Resource as ServerResource};
use super::user::Resource as UserResource;

#[derive(Debug, Clone, Copy)]
pub enum Resource {
    Server(ServerResource),
    Auth(AuthResource),
    User(UserResource),
    Organization(OrganizationResource),
    Project(ProjectResource),
}

impl From<ServerResource> for Resource {
    fn from(resource: ServerResource) -> Self {
        Self::Server(resource)
    }
}

impl From<AuthResource> for Resource {
    fn from(resource: AuthResource) -> Self {
        Self::Auth(resource)
    }
}

impl From<UserResource> for Resource {
    fn from(resource: UserResource) -> Self {
        Self::User(resource)
    }
}

impl From<OrganizationResource> for Resource {
    fn from(resource: OrganizationResource) -> Self {
        Self::Organization(resource)
    }
}

impl From<ProjectResource> for Resource {
    fn from(resource: ProjectResource) -> Self {
        Self::Project(resource)
    }
}

impl WordStr for Resource {
    fn singular(&self) -> &str {
        match self {
            Self::Server(server) => server.singular(),
            Self::Auth(auth) => auth.singular(),
            Self::User(user) => user.singular(),
            Self::Organization(org) => org.singular(),
            Self::Project(proj) => proj.singular(),
        }
    }

    fn plural(&self) -> &str {
        match self {
            Self::Server(server) => server.plural(),
            Self::Auth(auth) => auth.plural(),
            Self::User(user) => user.plural(),
            Self::Organization(org) => org.plural(),
            Self::Project(proj) => proj.plural(),
        }
    }
}
