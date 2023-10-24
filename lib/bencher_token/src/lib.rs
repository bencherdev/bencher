use bencher_json::Secret;
use once_cell::sync::Lazy;

mod audience;
mod claims;
mod error;
mod key;

pub use audience::Audience;
pub use claims::{Claims, InviteClaims, OrgClaims};
pub use error::TokenError;
pub use key::TokenKey;

#[cfg(debug_assertions)]
#[allow(clippy::expect_used)]
pub static DEFAULT_SECRET_KEY: Lazy<Secret> = Lazy::new(|| {
    "DO_NOT_USE_THIS_IN_PRODUCTION"
        .parse()
        .expect("Invalid secret key")
});
#[cfg(not(debug_assertions))]
pub static DEFAULT_SECRET_KEY: Lazy<Secret> = Lazy::new(|| uuid::Uuid::new_v4().into());
