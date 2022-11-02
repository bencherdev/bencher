use derive_more::Display;

use crate::WordStr;

pub mod confirm;
pub mod login;
pub mod signup;

// TODO Custom max TTL
// 30 minutes * 60 seconds / minute
pub const AUTH_TOKEN_TTL: u32 = 30 * 60;
// TODO Custom max TTL
// 30 days * 24 hours / day * 60 minutes / hour * 60 seconds / minute
pub const CLIENT_TOKEN_TTL: u32 = 30 * 24 * 60 * 60;

#[derive(Debug, Display, Clone, Copy)]
pub enum Resource {
    Confirm,
    Login,
    Signup,
}

impl WordStr for Resource {
    fn singular(&self) -> &str {
        match self {
            Self::Confirm => "confirmation",
            Self::Login => "login",
            Self::Signup => "signup",
        }
    }

    fn plural(&self) -> &str {
        match self {
            Self::Confirm => "confirmations",
            Self::Login => "logins",
            Self::Signup => "signups",
        }
    }
}
