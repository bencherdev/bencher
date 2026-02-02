use std::sync::LazyLock;

#[cfg(debug_assertions)]
use uuid as _;

use bencher_json::Secret;

mod audience;
mod claims;
mod error;
mod key;

pub use audience::Audience;
pub use claims::{
    Claims, InviteClaims, OAuthClaims, OciClaims, OciScopeClaims, OrgClaims, StateClaims,
};
pub use error::TokenError;
pub use key::TokenKey;

#[cfg(debug_assertions)]
#[expect(clippy::expect_used)]
pub static DEFAULT_SECRET_KEY: LazyLock<Secret> = LazyLock::new(|| {
    "DO_NOT_USE_THIS_IN_PRODUCTION"
        .parse()
        .expect("Invalid secret key")
});
#[cfg(not(debug_assertions))]
pub static DEFAULT_SECRET_KEY: LazyLock<Secret> = LazyLock::new(|| uuid::Uuid::new_v4().into());
