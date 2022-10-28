use derive_more::Display;

use crate::WordStr;

pub mod members;
pub mod organizations;

#[derive(Debug, Display, Clone, Copy)]
pub enum Resource {
    Member,
    Organization,
    OrganizationPermission,
}

impl WordStr for Resource {
    fn singular(&self) -> &str {
        match self {
            Self::Member => "member",
            Self::Organization => "organization",
            Self::OrganizationPermission => "organization permission",
        }
    }

    fn plural(&self) -> &str {
        match self {
            Self::Member => "members",
            Self::Organization => "organizations",
            Self::OrganizationPermission => "organization permissions",
        }
    }
}
