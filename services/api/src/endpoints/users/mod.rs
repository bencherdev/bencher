use bencher_macros::ToMethod;

pub mod tokens;

use tokens::Method as TokenMethod;

#[derive(Debug, Clone, Copy, ToMethod)]
pub enum Endpoint {
    Token(TokenMethod),
}

impl Endpoint {
    fn as_str(&self) -> &str {
        match self {
            Self::Token(_) => "token",
        }
    }
}
