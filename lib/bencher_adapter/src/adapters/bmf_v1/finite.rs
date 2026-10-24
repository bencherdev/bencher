use std::borrow::Cow;

use bencher_json::MeasureNameId;
use serde::{Deserialize, Deserializer, de::Error as _};

use crate::adapters::bmf_v1::leaf::Variant;

/// The input with each bare `NaN`, `Infinity`, and `-Infinity` outside a string, which a harness
/// writes for a value that is not finite but JSON has no token for, as `null`.
pub(crate) fn finite_json(input: &str) -> Cow<'_, str> {
    let bytes = input.as_bytes();
    let mut output = String::new();
    let (mut copied, mut index) = (0, 0);
    let (mut in_string, mut escaped) = (false, false);
    while let Some(byte) = bytes.get(index) {
        let token = (!in_string)
            .then(|| {
                [b"NaN".as_slice(), b"Infinity", b"-Infinity"]
                    .into_iter()
                    .find(|token| {
                        bytes
                            .get(index..)
                            .is_some_and(|rest| rest.starts_with(token))
                    })
            })
            .flatten();
        if let Some(token) = token {
            output.push_str(input.get(copied..index).unwrap_or_default());
            output.push_str("null");
            index += token.len();
            copied = index;
            continue;
        }
        if in_string {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'"' {
                in_string = false;
            }
        } else if *byte == b'"' {
            in_string = true;
        }
        index += 1;
    }
    if copied == 0 {
        return Cow::Borrowed(input);
    }
    output.push_str(input.get(copied..).unwrap_or_default());
    Cow::Owned(output)
}

/// A number, or none where a harness writes a string or `null` for a value that is not finite.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Finite(Option<f64>);

impl Finite {
    pub(crate) fn get(self) -> Option<f64> {
        self.0
    }

    /// The number times `factor` as `metric`, or counted as skipped when it is not finite.
    pub(crate) fn insert(
        self,
        variant: &mut Variant<'_>,
        measure: &MeasureNameId,
        metric: &str,
        factor: f64,
    ) {
        if let Some(value) = self.0 {
            variant.insert(measure.clone(), metric, value * factor);
        } else {
            variant.skip_metric();
        }
    }

    /// [`Self::insert`] for a spread of `mean`, such as an interval's bound, which is skipped
    /// uncounted when it is not finite beside a finite mean.
    pub(crate) fn insert_spread(
        self,
        mean: Self,
        variant: &mut Variant<'_>,
        measure: &MeasureNameId,
        metric: &str,
        factor: f64,
    ) {
        if self.0.is_some() || mean.0.is_none() {
            self.insert(variant, measure, metric, factor);
        }
    }

    pub(crate) fn map<F: FnOnce(f64) -> f64>(self, f: F) -> Self {
        Self(self.0.map(f))
    }
}

impl<'de> Deserialize<'de> for Finite {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match serde_json::Value::deserialize(deserializer)? {
            serde_json::Value::Number(number) => Ok(Self(number.as_f64())),
            serde_json::Value::String(_) | serde_json::Value::Null => Ok(Self(None)),
            other @ (serde_json::Value::Bool(_)
            | serde_json::Value::Array(_)
            | serde_json::Value::Object(_)) => Err(D::Error::custom(format!(
                "expected a number, found {other}"
            ))),
        }
    }
}
