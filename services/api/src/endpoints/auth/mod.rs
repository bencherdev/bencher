use derive_more::Display;

pub mod confirm;
pub mod invite;
pub mod login;
pub mod signup;

#[derive(Debug, Display, Clone, Copy)]
pub enum Resource {
    Confirm,
    Invite,
    Login,
    Signup,
}
