#[cfg(feature = "schema")]
use schemars::JsonSchema;

use serde::{Deserialize, Serialize};

/// An OAuth refresh token, exchanged at the token endpoint for a new access token.
#[typeshare::typeshare]
#[derive(Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(try_from = "String")]
pub struct OAuthRefreshToken(String);

crate::keys::api_key_impl!(
    OAuthRefreshToken,
    prefix = "bencher_refresh_",
    error = OAuthRefreshToken
);

crate::keys::api_key_tests!(
    OAuthRefreshToken,
    sample = "bencher_refresh_aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh",
);
