use std::fmt;

use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;

use serde::{
    Deserialize, Deserializer, Serialize,
    de::{self, Visitor},
};

#[derive(Debug, Display, Clone, Copy, PartialEq, PartialOrd, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct RecaptchaScore(f32);

impl<'de> Deserialize<'de> for RecaptchaScore {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_f64(RecaptchaScoreVisitor)
    }
}

struct RecaptchaScoreVisitor;

impl Visitor<'_> for RecaptchaScoreVisitor {
    type Value = RecaptchaScore;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a valid recaptcha score between 0.0 and 1.0")
    }

    #[expect(clippy::cast_possible_truncation)]
    fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_f32(v as f32)
    }

    fn visit_f32<E>(self, v: f32) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if !(0.0..=1.0).contains(&v) {
            return Err(E::custom(format!(
                "recaptcha score must be between 0.0 and 1.0, got {v}"
            )));
        }
        Ok(RecaptchaScore(v))
    }
}
