use bencher_macros::ToMethod;

pub mod tokens;

use tokens::Method as TokenMethod;

use crate::{util::endpoint::into_endpoint, WordStr};

#[derive(Debug, Clone, Copy, ToMethod)]
pub enum Resource {
    Token(TokenMethod),
}

impl WordStr for Resource {
    fn singular(&self) -> &str {
        match self {
            Self::Token(_) => "token",
        }
    }

    fn plural(&self) -> &str {
        match self {
            Self::Token(_) => "tokens",
        }
    }
}

into_endpoint!(Users, Resource);

impl Resource {
    fn as_str(&self) -> &str {
        match self {
            Self::Token(_) => "token",
        }
    }
}
