#[cfg(feature = "schema")]
use schemars::JsonSchema;

use serde::{Deserialize, Serialize};

#[typeshare::typeshare]
#[derive(Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(try_from = "String")]
pub struct UserKey(String);

crate::keys::api_key_impl!(UserKey, prefix = "bencher_user_", error = UserKey);

crate::keys::api_key_tests!(
    UserKey,
    sample = "bencher_user_aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh",
);
