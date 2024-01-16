pub mod accept;
pub mod confirm;
#[cfg(feature = "plus")]
pub mod github;
pub mod login;
pub mod signup;

// TODO Custom max TTL
// 30 minutes * 60 seconds / minute
pub const AUTH_TOKEN_TTL: u32 = 30 * 60;
// TODO Custom max TTL
// 30 days * 24 hours / day * 60 minutes / hour * 60 seconds / minute
pub const CLIENT_TOKEN_TTL: u32 = 30 * 24 * 60 * 60;

#[cfg(feature = "plus")]
pub const PLAN_ARG: &str = "plan";
pub const TOKEN_ARG: &str = "token";
