use std::{fmt, str::FromStr};

use oso::{PolarClass, PolarValue, ToPolar};

pub const VIEWER_ROLE: &str = "viewer";
pub const DEVELOPER_ROLE: &str = "developer";
pub const MAINTAINER_ROLE: &str = "maintainer";

pub const VIEW_PERM: &str = "view";
pub const CREATE_PERM: &str = "create";
pub const EDIT_PERM: &str = "edit";
pub const DELETE_PERM: &str = "delete";
pub const MANAGE_PERM: &str = "manage";
pub const CREATE_ROLE_ASSIGNMENTS_PERM: &str = "create_role_assignments";
pub const LIST_ROLE_ASSIGNMENTS_PERM: &str = "list_role_assignments";
pub const UPDATE_ROLE_ASSIGNMENTS_PERM: &str = "update_role_assignments";
pub const DELETE_ROLE_ASSIGNMENTS_PERM: &str = "delete_role_assignments";

#[derive(Debug, Clone, PolarClass)]
pub struct Project {
    #[polar(attribute)]
    pub uuid: String,
    #[polar(attribute)]
    pub parent: String,
}

#[derive(Debug, Clone, Copy)]
pub enum Role {
    Viewer,
    Developer,
    Maintainer,
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Viewer => VIEWER_ROLE,
                Self::Developer => DEVELOPER_ROLE,
                Self::Maintainer => MAINTAINER_ROLE,
            }
        )
    }
}

impl ToPolar for Role {
    fn to_polar(self) -> PolarValue {
        PolarValue::String(self.to_string())
    }
}

impl FromStr for Role {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            VIEWER_ROLE => Ok(Role::Viewer),
            DEVELOPER_ROLE => Ok(Role::Developer),
            MAINTAINER_ROLE => Ok(Role::Maintainer),
            _ => Err(s.into()),
        }
    }
}

#[derive(Clone, Copy)]
pub enum Permission {
    View,
    Create,
    Edit,
    Delete,
    Manage,
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
                Self::View => VIEW_PERM,
                Self::Create => CREATE_PERM,
                Self::Edit => EDIT_PERM,
                Self::Delete => DELETE_PERM,
                Self::Manage => MANAGE_PERM,
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
