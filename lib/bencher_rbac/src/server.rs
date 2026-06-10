use std::fmt;

use oso::{PolarClass, PolarValue, ToPolar};

const LOCKED_ROLE: &str = "locked";
const USER_ROLE: &str = "user";
const ADMIN_ROLE: &str = "admin";

pub const SESSION_PEM: &str = "session";
pub const ADMINISTER_PEM: &str = "administer";

#[derive(Debug, Clone, Copy, PolarClass)]
pub struct Server {}

#[derive(Debug, Clone, Copy)]
pub enum Role {
    Locked,
    User,
    Admin,
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Locked => LOCKED_ROLE,
                Self::User => USER_ROLE,
                Self::Admin => ADMIN_ROLE,
            }
        )
    }
}

impl ToPolar for Role {
    fn to_polar(self) -> PolarValue {
        PolarValue::String(self.to_string())
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Permission {
    Session,
    Administer,
}

impl fmt::Display for Permission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Session => SESSION_PEM,
                Self::Administer => ADMINISTER_PEM,
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
    use oso::{PolarValue, ToPolar as _};
    use pretty_assertions::assert_eq;

    use super::{ADMINISTER_PEM, Permission, Role, SESSION_PEM};

    /// Every server `Role` variant paired with its expected string.
    /// Note: server `Role` has no `FromStr` impl — it is write-only to Polar.
    const ROLES: [(Role, &str); 3] = [
        (Role::Locked, "locked"),
        (Role::User, "user"),
        (Role::Admin, "admin"),
    ];

    /// Every server `Permission` variant paired with its expected string.
    const PERMISSIONS: [(Permission, &str); 2] = [
        (Permission::Session, SESSION_PEM),
        (Permission::Administer, ADMINISTER_PEM),
    ];

    #[test]
    fn server_role_display_matches_expected() {
        for (role, expected) in ROLES {
            assert_eq!(role.to_string(), expected);
        }
    }

    #[test]
    fn server_role_to_polar() {
        for (role, expected) in ROLES {
            assert_eq!(role.to_polar(), PolarValue::String(expected.to_owned()));
        }
    }

    #[test]
    fn server_permission_display_matches_expected() {
        assert_eq!(SESSION_PEM, "session");
        assert_eq!(ADMINISTER_PEM, "administer");
        for (permission, expected) in PERMISSIONS {
            assert_eq!(permission.to_string(), expected);
        }
    }

    #[test]
    fn server_permission_to_polar() {
        for (permission, expected) in PERMISSIONS {
            assert_eq!(
                permission.to_polar(),
                PolarValue::String(expected.to_owned())
            );
        }
    }
}
