use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::{fmt, str::FromStr};
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer, Serialize,
};

use crate::ValidError;

const MAX_BENCHMARK_NAME_LEN: usize = 1024;

#[derive(Debug, Display, Clone, Eq, PartialEq, Hash, Serialize, Default)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct BenchmarkName(String);

impl BenchmarkName {
    pub fn try_push(&mut self, separator: char, other: &Self) -> Result<(), ValidError> {
        let remaining_capacity = MAX_BENCHMARK_NAME_LEN - self.0.len();
        if other.0.len() < remaining_capacity {
            self.0.push(separator);
            self.0.push_str(other.as_ref());
            assert!(self.0.len() <= MAX_BENCHMARK_NAME_LEN);
            Ok(())
        } else {
            Err(ValidError::BenchmarkName(format!("{}.{}", self.0, other.0)))
        }
    }
}

impl FromStr for BenchmarkName {
    type Err = ValidError;

    fn from_str(benchmark_name: &str) -> Result<Self, Self::Err> {
        if is_valid_benchmark_name(benchmark_name) {
            Ok(Self(benchmark_name.into()))
        } else {
            Err(ValidError::BenchmarkName(benchmark_name.into()))
        }
    }
}

impl AsRef<str> for BenchmarkName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<BenchmarkName> for String {
    fn from(benchmark_name: BenchmarkName) -> Self {
        benchmark_name.0
    }
}

impl<'de> Deserialize<'de> for BenchmarkName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(BenchmarkNameVisitor)
    }
}

struct BenchmarkNameVisitor;

impl<'de> Visitor<'de> for BenchmarkNameVisitor {
    type Value = BenchmarkName;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a valid benchmark name")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value.parse().map_err(E::custom)
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_benchmark_name(benchmark_name: &str) -> bool {
    !benchmark_name.is_empty() && benchmark_name.len() <= MAX_BENCHMARK_NAME_LEN
}

#[cfg(test)]
mod test {
    use crate::BenchmarkName;

    use super::{is_valid_benchmark_name, MAX_BENCHMARK_NAME_LEN};
    use pretty_assertions::assert_eq;

    #[test]
    fn test_benchmark_name() {
        assert_eq!(true, is_valid_benchmark_name("a"));
        assert_eq!(true, is_valid_benchmark_name("ab"));
        assert_eq!(true, is_valid_benchmark_name("abc"));
        assert_eq!(true, is_valid_benchmark_name("ABC"));
        assert_eq!(true, is_valid_benchmark_name("abc ~ABC!"));

        assert_eq!(false, is_valid_benchmark_name(""));
    }

    #[test]
    fn test_benchmark_name_try_push_ok() {
        let mut benchmark_name: BenchmarkName = "0123456789".parse().unwrap();
        let benchmark_name_len = benchmark_name.0.len();
        assert_eq!(benchmark_name_len, 10);

        let other_benchmark_name_bytes: [u8; MAX_BENCHMARK_NAME_LEN - 11] =
            [0; MAX_BENCHMARK_NAME_LEN - 11];
        let other_benchmark_name: BenchmarkName = std::str::from_utf8(&other_benchmark_name_bytes)
            .unwrap()
            .parse()
            .unwrap();
        let other_benchmark_name_len = other_benchmark_name.0.len();

        // 10 + 1 + 1013 = 1024
        assert_eq!(
            benchmark_name_len + 1 + other_benchmark_name_len,
            MAX_BENCHMARK_NAME_LEN
        );

        benchmark_name.try_push('.', &other_benchmark_name).unwrap();
        assert_eq!(benchmark_name.0.len(), MAX_BENCHMARK_NAME_LEN);
        is_valid_benchmark_name(&benchmark_name.0);
        assert_eq!(other_benchmark_name_len, other_benchmark_name.0.len());
    }

    #[test]
    fn test_benchmark_name_try_push_err() {
        let mut benchmark_name: BenchmarkName = "0123456789".parse().unwrap();
        let benchmark_name_len = benchmark_name.0.len();
        assert_eq!(benchmark_name_len, 10);

        let other_benchmark_name_bytes: [u8; MAX_BENCHMARK_NAME_LEN - 10] =
            [0; MAX_BENCHMARK_NAME_LEN - 10];
        let other_benchmark_name: BenchmarkName = std::str::from_utf8(&other_benchmark_name_bytes)
            .unwrap()
            .parse()
            .unwrap();
        let other_benchmark_name_len = other_benchmark_name.0.len();

        // 10 + 1 + 1014 = 1025
        assert_eq!(
            benchmark_name_len + 1 + other_benchmark_name_len,
            MAX_BENCHMARK_NAME_LEN + 1
        );

        assert!(benchmark_name.try_push('.', &other_benchmark_name).is_err());
        assert_eq!(benchmark_name_len, benchmark_name.0.len());
        assert_eq!(other_benchmark_name_len, other_benchmark_name.0.len());
    }
}
