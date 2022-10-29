use derive_more::Display;

use crate::WordStr;

pub mod allowed;
pub mod members;
pub mod organizations;
pub mod projects;

#[derive(Debug, Display, Clone, Copy)]
pub enum Resource {
    Member,
    Organization,
    OrganizationPermission,
    Project,
}

impl WordStr for Resource {
    fn singular(&self) -> &str {
        match self {
            Self::Member => "member",
            Self::Organization => "organization",
            Self::OrganizationPermission => "organization permission",
            Self::Project => "project",
        }
    }

    fn plural(&self) -> &str {
        match self {
            Self::Member => "members",
            Self::Organization => "organizations",
            Self::OrganizationPermission => "organization permissions",
            Self::Project => "projects",
        }
    }
}
