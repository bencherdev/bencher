use std::{fmt, str::FromStr};

use oso::{PolarClass, PolarValue, ToPolar};

use crate::{
    CREATE_PERM, CREATE_ROLE_PERM, DELETE_PERM, DELETE_ROLE_PERM, EDIT_PERM, EDIT_ROLE_PERM,
    MANAGE_PERM, VIEW_PERM, VIEW_ROLE_PERM,
};

pub const VIEWER_ROLE: &str = "viewer";
pub const DEVELOPER_ROLE: &str = "developer";
pub const MAINTAINER_ROLE: &str = "maintainer";

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

#[derive(Debug, Clone, Copy)]
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
                Self::CreateRoleAssignments => CREATE_ROLE_PERM,
                Self::ListRoleAssignments => VIEW_ROLE_PERM,
                Self::UpdateRoleAssignments => EDIT_ROLE_PERM,
                Self::DeleteRoleAssignments => DELETE_ROLE_PERM,
            }
        )
    }
}

impl ToPolar for Permission {
    fn to_polar(self) -> PolarValue {
        PolarValue::String(self.to_string())
    }
}
