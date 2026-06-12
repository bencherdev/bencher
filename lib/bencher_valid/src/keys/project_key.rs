#[cfg(feature = "schema")]
use schemars::JsonSchema;

use serde::{Deserialize, Serialize};

#[typeshare::typeshare]
#[derive(Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(try_from = "String")]
pub struct ProjectKey(String);

crate::keys::api_key_impl!(ProjectKey, prefix = "bencher_run_", error = ProjectKey);

crate::keys::api_key_tests!(
    ProjectKey,
    sample = "bencher_run_aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh",
);
