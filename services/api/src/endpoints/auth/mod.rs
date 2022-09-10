use derive_more::Display;

pub mod confirm;
pub mod invite;
pub mod login;
pub mod signup;

#[derive(Debug, Display, Clone, Copy)]
pub enum Endpoint {
    Confirm,
    Invite,
    Login,
    Signup,
}
