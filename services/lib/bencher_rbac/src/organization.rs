use std::{fmt, str::FromStr};

use bencher_json::organization::member::{LEADER_ROLE, MEMBER_ROLE};
use oso::{PolarClass, PolarValue, ToPolar};

use crate::{
    CREATE_PERM, CREATE_ROLE_PERM, DELETE_PERM, DELETE_ROLE_PERM, EDIT_PERM, EDIT_ROLE_PERM,
    MANAGE_PERM, VIEW_PERM, VIEW_ROLE_PERM,
};

#[derive(Debug, Clone, PolarClass)]
pub struct Organization {
    #[polar(attribute)]
    pub id: String,
}

#[derive(Debug, Clone, Copy)]
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

impl ToPolar for Role {
    fn to_polar(self) -> PolarValue {
        PolarValue::String(self.to_string())
    }
}

impl FromStr for Role {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            MEMBER_ROLE => Ok(Self::Member),
            LEADER_ROLE => Ok(Self::Leader),
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
    ViewRole,
    CreateRole,
    EditRole,
    DeleteRole,
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
                Self::ViewRole => VIEW_ROLE_PERM,
                Self::CreateRole => CREATE_ROLE_PERM,
                Self::EditRole => EDIT_ROLE_PERM,
                Self::DeleteRole => DELETE_ROLE_PERM,
            }
        )
    }
}

impl ToPolar for Permission {
    fn to_polar(self) -> PolarValue {
        PolarValue::String(self.to_string())
    }
}
