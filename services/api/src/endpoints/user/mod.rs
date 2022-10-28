pub mod tokens;

use derive_more::Display;

use crate::WordStr;

#[derive(Debug, Display, Clone, Copy)]
pub enum Resource {
    Token,
}

impl WordStr for Resource {
    fn singular(&self) -> &str {
        match self {
            Self::Token => "token",
        }
    }

    fn plural(&self) -> &str {
        match self {
            Self::Token => "tokens",
        }
    }
}
