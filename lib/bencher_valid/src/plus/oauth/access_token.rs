#[cfg(feature = "schema")]
use schemars::JsonSchema;

use serde::{Deserialize, Serialize};

/// An OAuth access token, accepted only by the MCP server.
#[typeshare::typeshare]
#[derive(Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(try_from = "String")]
pub struct OAuthAccessToken(String);

crate::keys::api_key_impl!(
    OAuthAccessToken,
    prefix = "bencher_oauth_",
    error = OAuthAccessToken
);

crate::keys::api_key_tests!(
    OAuthAccessToken,
    sample = "bencher_oauth_aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh",
);
