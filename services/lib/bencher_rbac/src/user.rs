use std::collections::HashMap;

use oso::PolarClass;

// TODO once it supported by PolarClass, switch over to UUIDs as the HashMap keys
#[derive(Debug, Clone, PolarClass)]
pub struct User {
    #[polar(attribute)]
    pub admin: bool,
    #[polar(attribute)]
    pub locked: bool,
    #[polar(attribute)]
    pub organizations: OrganizationRoles,
    #[polar(attribute)]
    pub projects: ProjectRoles,
}

pub type OrganizationRoles = HashMap<String, crate::organization::Role>;
pub type ProjectRoles = HashMap<String, crate::project::Role>;
