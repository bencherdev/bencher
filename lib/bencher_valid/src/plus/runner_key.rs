#[cfg(feature = "schema")]
use schemars::JsonSchema;

use serde::{Deserialize, Serialize};

#[typeshare::typeshare]
#[derive(Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(try_from = "String")]
pub struct RunnerKey(String);

crate::keys::api_key_impl!(RunnerKey, prefix = "bencher_runner_", error = RunnerKey);

crate::keys::api_key_tests!(
    RunnerKey,
    sample = "bencher_runner_aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh",
);
