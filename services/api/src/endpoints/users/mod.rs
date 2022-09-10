use bencher_macros::ToMethod;

pub mod tokens;

use tokens::Method as TokenMethod;

#[derive(Debug, Clone, Copy, ToMethod)]
pub enum Endpoint {
    Token(TokenMethod),
}
