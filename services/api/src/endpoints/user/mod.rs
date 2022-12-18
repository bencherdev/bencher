pub mod tokens;
pub mod users;

use derive_more::Display;

use crate::WordStr;

#[derive(Debug, Display, Clone, Copy)]
pub enum Resource {
    User,
    Token,
}

impl WordStr for Resource {
    fn singular(&self) -> &str {
        match self {
            Self::User => "user",
            Self::Token => "token",
        }
    }

    fn plural(&self) -> &str {
        match self {
            Self::User => "users",
            Self::Token => "tokens",
        }
    }
}
