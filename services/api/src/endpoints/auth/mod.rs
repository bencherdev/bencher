use derive_more::Display;

use crate::WordStr;

pub mod confirm;
pub mod invite;
pub mod login;
pub mod signup;

#[derive(Debug, Display, Clone, Copy)]
pub enum Resource {
    Confirm,
    Invite,
    Login,
    Signup,
}

impl WordStr for Resource {
    fn singular(&self) -> &str {
        match self {
            Self::Confirm => "confirmation",
            Self::Invite => "invite",
            Self::Login => "login",
            Self::Signup => "signup",
        }
    }

    fn plural(&self) -> &str {
        match self {
            Self::Confirm => "confirmations",
            Self::Invite => "invites",
            Self::Login => "logins",
            Self::Signup => "signups",
        }
    }
}
