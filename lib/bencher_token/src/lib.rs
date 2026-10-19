use std::sync::LazyLock;

use bencher_json::Secret;

mod audience;
mod claims;
mod error;
mod key;
#[cfg(feature = "plus")]
mod oauth;

pub use audience::Audience;
pub use claims::{
    AuthOciClaims, Claims, EmailChangeClaims, InviteClaims, OAuthClaims, OciAction, OciScopeClaims,
    OrgClaims, PublicOciClaims, StateClaims,
};
#[cfg(feature = "plus")]
pub use claims::{ProjectOciClaims, RunnerOciClaims};
pub use error::TokenError;
pub use key::TokenKey;
#[cfg(feature = "plus")]
pub use oauth::{OAuthSigningKey, OAuthSigningKeyError};

#[cfg(debug_assertions)]
#[expect(clippy::expect_used, reason = "debug-only hardcoded key must be valid")]
pub static DEFAULT_SECRET_KEY: LazyLock<Secret> = LazyLock::new(|| {
    "DO_NOT_USE_THIS_IN_PRODUCTION"
        .parse()
        .expect("Invalid secret key")
});
#[cfg(not(debug_assertions))]
pub static DEFAULT_SECRET_KEY: LazyLock<Secret> = LazyLock::new(|| uuid::Uuid::new_v4().into());
