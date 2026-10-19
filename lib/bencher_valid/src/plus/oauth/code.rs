#[cfg(feature = "schema")]
use schemars::JsonSchema;

use serde::{Deserialize, Serialize};

/// An OAuth authorization code, exchanged once at the token endpoint.
#[typeshare::typeshare]
#[derive(Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(try_from = "String")]
pub struct OAuthCode(String);

crate::keys::api_key_impl!(OAuthCode, prefix = "bencher_code_", error = OAuthCode);

crate::keys::api_key_tests!(
    OAuthCode,
    sample = "bencher_code_aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh",
);
