use bencher_macros::ToMethod;

pub mod tokens;

use tokens::Method as TokenMethod;

use crate::IntoEndpoint;

#[derive(Debug, Clone, Copy, ToMethod)]
pub enum Endpoint {
    Token(TokenMethod),
}

impl IntoEndpoint for Endpoint {
    fn into_endpoint(self) -> crate::Endpoint {
        crate::Endpoint::Users(self)
    }
}

impl Endpoint {
    fn as_str(&self) -> &str {
        match self {
            Self::Token(_) => "token",
        }
    }
}
