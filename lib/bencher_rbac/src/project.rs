use std::{fmt, str::FromStr};

use oso::{PolarClass, PolarValue, ToPolar};

use crate::{
    CREATE_PERM, CREATE_ROLE_PERM, DELETE_PERM, DELETE_ROLE_PERM, EDIT_PERM, EDIT_ROLE_PERM,
    MANAGE_PERM, VIEW_PERM, VIEW_ROLE_PERM,
};

const VIEWER_ROLE: &str = "viewer";
const DEVELOPER_ROLE: &str = "developer";
const MAINTAINER_ROLE: &str = "maintainer";

#[derive(Debug, Clone, PolarClass)]
pub struct Project {
    #[polar(attribute)]
    pub id: String,
    #[polar(attribute)]
    pub organization_id: String,
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
            VIEWER_ROLE => Ok(Self::Viewer),
            DEVELOPER_ROLE => Ok(Self::Developer),
            MAINTAINER_ROLE => Ok(Self::Maintainer),
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

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;

    use oso::{PolarValue, ToPolar as _};
    use pretty_assertions::assert_eq;

    use super::{Permission, Role};

    /// Every project `Role` variant paired with its expected string.
    const ROLES: [(Role, &str); 3] = [
        (Role::Viewer, "viewer"),
        (Role::Developer, "developer"),
        (Role::Maintainer, "maintainer"),
    ];

    /// Every project `Permission` variant paired with its expected string.
    const PERMISSIONS: [(Permission, &str); 9] = [
        (Permission::View, "view"),
        (Permission::Create, "create"),
        (Permission::Edit, "edit"),
        (Permission::Delete, "delete"),
        (Permission::Manage, "manage"),
        (Permission::ViewRole, "view_role"),
        (Permission::CreateRole, "create_role"),
        (Permission::EditRole, "edit_role"),
        (Permission::DeleteRole, "delete_role"),
    ];

    #[test]
    fn project_role_display_matches_expected() {
        for (role, expected) in ROLES {
            assert_eq!(role.to_string(), expected);
        }
    }

    #[test]
    fn project_role_from_str_round_trip() {
        for (_, expected) in ROLES {
            let role = Role::from_str(expected).unwrap();
            assert_eq!(role.to_string(), expected);
        }
    }

    #[test]
    fn project_role_from_str_invalid() {
        for invalid in ["", "member", "leader", "Viewer", "MAINTAINER", " viewer"] {
            let error = Role::from_str(invalid).unwrap_err();
            assert_eq!(error, invalid);
        }
    }

    #[test]
    fn project_role_to_polar() {
        for (role, expected) in ROLES {
            assert_eq!(role.to_polar(), PolarValue::String(expected.to_owned()));
        }
    }

    #[test]
    fn project_permission_display_matches_expected() {
        for (permission, expected) in PERMISSIONS {
            assert_eq!(permission.to_string(), expected);
        }
    }

    #[test]
    fn project_permission_to_polar() {
        for (permission, expected) in PERMISSIONS {
            assert_eq!(
                permission.to_polar(),
                PolarValue::String(expected.to_owned())
            );
        }
    }
}
