use std::str::FromStr;

use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ValidError;

/// A quarter of the benchmark name limit, so a prefixed name keeps most of the limit for itself.
const MAX_BENCHMARK_PREFIX_LEN: usize = 256;

/// The string put in front of every benchmark name a report holds, joined with no delimiter.
#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(try_from = "String")]
pub struct BenchmarkPrefix(String);

impl TryFrom<String> for BenchmarkPrefix {
    type Error = ValidError;

    fn try_from(prefix: String) -> Result<Self, Self::Error> {
        if !prefix.is_empty() && prefix.len() <= MAX_BENCHMARK_PREFIX_LEN {
            Ok(Self(prefix))
        } else {
            Err(ValidError::BenchmarkPrefix(prefix))
        }
    }
}

impl FromStr for BenchmarkPrefix {
    type Err = ValidError;

    fn from_str(prefix: &str) -> Result<Self, Self::Err> {
        Self::try_from(prefix.to_owned())
    }
}

impl AsRef<str> for BenchmarkPrefix {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<BenchmarkPrefix> for String {
    fn from(prefix: BenchmarkPrefix) -> Self {
        prefix.0
    }
}

#[cfg(test)]
mod tests {
    use super::{BenchmarkPrefix, MAX_BENCHMARK_PREFIX_LEN};

    /// Kills: a limit off by one in either direction, a limit in characters rather than the bytes
    /// a benchmark name counts, and a prefix with no limit.
    #[test]
    fn benchmark_prefix_holds_up_to_256_bytes() {
        let at_limit = serde_json::json!("a".repeat(MAX_BENCHMARK_PREFIX_LEN));
        serde_json::from_value::<BenchmarkPrefix>(at_limit).expect("a prefix at the limit");
        // 129 characters of 2 bytes each.
        for past_limit in ["a".repeat(MAX_BENCHMARK_PREFIX_LEN + 1), "é".repeat(129)] {
            serde_json::from_value::<BenchmarkPrefix>(serde_json::json!(past_limit))
                .expect_err("a prefix past the limit");
        }
    }
}
