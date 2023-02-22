use derive_more::Display;

use crate::WordStr;

pub mod allowed;
pub mod members;
pub mod organizations;
#[cfg(feature = "plus")]
pub mod plan;
pub mod projects;
#[cfg(feature = "plus")]
pub mod usage;

#[derive(Debug, Display, Clone, Copy)]
pub enum Resource {
    Member,
    Organization,
    OrganizationPermission,
    Project,
    #[cfg(feature = "plus")]
    Plan,
    #[cfg(feature = "plus")]
    Usage,
}

impl WordStr for Resource {
    fn singular(&self) -> &str {
        match self {
            Self::Member => "member",
            Self::Organization => "organization",
            Self::OrganizationPermission => "organization permission",
            Self::Project => "project",
            #[cfg(feature = "plus")]
            Self::Plan => "plan",
            #[cfg(feature = "plus")]
            Self::Usage => "usage",
        }
    }

    fn plural(&self) -> &str {
        match self {
            Self::Member => "members",
            Self::Organization => "organizations",
            Self::OrganizationPermission => "organization permissions",
            Self::Project => "projects",
            #[cfg(feature = "plus")]
            Self::Plan => "plans",
            #[cfg(feature = "plus")]
            Self::Usage => "usages",
        }
    }
}
