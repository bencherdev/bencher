use bencher_macros::ToMethod;

pub mod tokens;

use tokens::Method as TokenMethod;

use crate::util::endpoint::into_endpoint;

#[derive(Debug, Clone, Copy, ToMethod)]
pub enum Resource {
    Token(TokenMethod),
}

into_endpoint!(Users, Resource);

impl Resource {
    fn as_str(&self) -> &str {
        match self {
            Self::Token(_) => "token",
        }
    }
}
