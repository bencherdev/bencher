use std::fmt;

use oso::{PolarClass, PolarValue, ToPolar};

pub const LOCKED_ROLE: &str = "locked";
pub const USER_ROLE: &str = "user";
pub const ADMIN_ROLE: &str = "admin";

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
