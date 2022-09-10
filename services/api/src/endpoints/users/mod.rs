pub mod tokens;

use crate::{util::endpoint::into_endpoint, WordStr};

#[derive(Debug, Clone, Copy)]
pub enum Resource {
    Token,
}

into_endpoint!(Users, Resource);

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
