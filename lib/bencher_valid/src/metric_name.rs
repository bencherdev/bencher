use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::str::FromStr;
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::wasm_bindgen;

use serde::{Deserialize, Serialize};

use crate::ValidError;

/// The name of a single scalar inside a metric.
///
/// Every name is equal on the wire. `value`, `lower_value`, and `upper_value` are
/// conventional, not privileged: they are the names the metric triple maps onto,
/// and the names the console knows well enough to draw a band.
#[derive(Debug, Display, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(try_from = "String")]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
pub struct MetricName(String);

#[cfg(feature = "db")]
crate::typed_string!(MetricName);

impl MetricName {
    pub const MAX_LEN: usize = crate::MAX_LEN;

    /// The point estimate. A real name like any other, not a sentinel.
    #[must_use]
    pub fn value() -> Self {
        Self(VALUE.to_owned())
    }

    /// The lower bound of the metric triple.
    #[must_use]
    pub fn lower_value() -> Self {
        Self(LOWER_VALUE.to_owned())
    }

    /// The upper bound of the metric triple.
    #[must_use]
    pub fn upper_value() -> Self {
        Self(UPPER_VALUE.to_owned())
    }
}

/// The conventional name of the point estimate.
pub const VALUE: &str = "value";
/// The conventional name of the lower bound.
pub const LOWER_VALUE: &str = "lower_value";
/// The conventional name of the upper bound.
pub const UPPER_VALUE: &str = "upper_value";

impl TryFrom<String> for MetricName {
    type Error = ValidError;

    fn try_from(metric_name: String) -> Result<Self, Self::Error> {
        if is_valid_metric_name(&metric_name) {
            Ok(Self(metric_name))
        } else {
            Err(ValidError::MetricName(metric_name))
        }
    }
}

impl FromStr for MetricName {
    type Err = ValidError;

    fn from_str(metric_name: &str) -> Result<Self, Self::Err> {
        Self::try_from(metric_name.to_owned())
    }
}

impl AsRef<str> for MetricName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<MetricName> for String {
    fn from(metric_name: MetricName) -> Self {
        metric_name.0
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_metric_name(metric_name: &str) -> bool {
    crate::is_valid_non_empty(metric_name)
}

#[cfg(test)]
mod tests {
    use crate::tests::{LEN_0_STR, LEN_64_STR, LEN_65_STR};

    use super::{MetricName, is_valid_metric_name};
    use pretty_assertions::assert_eq;

    #[test]
    fn is_valid_metric_name_true() {
        for value in [
            "value",
            "lower_value",
            "upper_value",
            "p99",
            "p50",
            LEN_64_STR,
            LEN_65_STR,
        ] {
            assert_eq!(true, is_valid_metric_name(value), "{value}");
        }
    }

    #[test]
    fn is_valid_metric_name_false() {
        assert_eq!(false, is_valid_metric_name(LEN_0_STR));
    }

    #[test]
    fn conventional_names() {
        assert_eq!(MetricName::value().as_ref(), "value");
        assert_eq!(MetricName::lower_value().as_ref(), "lower_value");
        assert_eq!(MetricName::upper_value().as_ref(), "upper_value");
    }

    #[test]
    fn metric_name_serde_roundtrip() {
        let metric_name: MetricName = serde_json::from_str("\"p99\"").unwrap();
        assert_eq!(metric_name.as_ref(), "p99");
        let json = serde_json::to_string(&metric_name).unwrap();
        assert_eq!(json, "\"p99\"");

        serde_json::from_str::<MetricName>("\"\"").unwrap_err();
    }
}
