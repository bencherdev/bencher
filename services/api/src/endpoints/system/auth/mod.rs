pub mod confirm;
pub mod login;
pub mod signup;

// TODO Custom max TTL
// 30 minutes * 60 seconds / minute
pub const AUTH_TOKEN_TTL: u32 = 30 * 60;
// TODO Custom max TTL
// 30 days * 24 hours / day * 60 minutes / hour * 60 seconds / minute
pub const CLIENT_TOKEN_TTL: u32 = 30 * 24 * 60 * 60;

#[cfg(feature = "plus")]
const PLAN_ARG: &str = "plan";
const TOKEN_ARG: &str = "token";
