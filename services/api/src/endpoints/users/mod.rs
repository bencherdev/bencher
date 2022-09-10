use derive_more::Display;

pub mod tokens;

#[derive(Debug, Display, Clone, Copy)]
pub enum Endpoint {
    Token,
}
