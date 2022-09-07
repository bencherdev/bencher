use std::fmt;

use oso::{PolarClass, PolarValue, ToPolar};

pub const MEMBER_ROLE: &str = "member";
pub const LEADER_ROLE: &str = "leader";

pub const READ_PERM: &str = "read";
pub const CREATE_PROJECTS_PERM: &str = "create_projects";
pub const LIST_PROJECTS_PERM: &str = "list_projects";
pub const CREATE_ROLE_ASSIGNMENTS_PERM: &str = "create_role_assignments";
pub const LIST_ROLE_ASSIGNMENTS_PERM: &str = "list_role_assignments";
pub const UPDATE_ROLE_ASSIGNMENTS_PERM: &str = "update_role_assignments";
pub const DELETE_ROLE_ASSIGNMENTS_PERM: &str = "delete_role_assignments";

#[derive(Clone, PolarClass)]
pub struct Organization {
    #[polar(attribute)]
    pub uuid: String,
}

#[derive(Clone, Copy)]
pub enum Role {
    Member,
    Leader,
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Member => MEMBER_ROLE,
                Self::Leader => LEADER_ROLE,
            }
        )
    }
}


#[derive(Clone, Copy)]
pub enum Permission {
    Read,
    CreateProjects,
    ListProjects,
    CreateRoleAssignments,
    ListRoleAssignments,
    UpdateRoleAssignments,
    DeleteRoleAssignments,
}

impl fmt::Display for Permission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Read => READ_PERM,
                Self::CreateProjects => CREATE_PROJECTS_PERM,
                Self::ListProjects => LIST_PROJECTS_PERM,
                Self::CreateRoleAssignments => CREATE_ROLE_ASSIGNMENTS_PERM,
                Self::ListRoleAssignments => LIST_ROLE_ASSIGNMENTS_PERM,
                Self::UpdateRoleAssignments => UPDATE_ROLE_ASSIGNMENTS_PERM,
                Self::DeleteRoleAssignments => DELETE_ROLE_ASSIGNMENTS_PERM,
            }
        )
    }
}

impl ToPolar for Permission {
    fn to_polar(self) -> PolarValue {
        PolarValue::String(self.to_string())
    }
}
