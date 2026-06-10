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

    #[expect(
        clippy::cast_possible_truncation,
        reason = "score is 0.0..=1.0, fits in f32"
    )]
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

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use serde::{
        Deserialize as _,
        de::value::{Error as DeserializeError, F32Deserializer, F64Deserializer},
    };

    use super::RecaptchaScore;

    const MIN_SCORE: f32 = 0.0;
    const MID_SCORE: f32 = 0.5;
    const MAX_SCORE: f32 = 1.0;

    fn score_from_f64(score: f64) -> Result<RecaptchaScore, DeserializeError> {
        RecaptchaScore::deserialize(F64Deserializer::new(score))
    }

    fn score_from_f32(score: f32) -> Result<RecaptchaScore, DeserializeError> {
        RecaptchaScore::deserialize(F32Deserializer::new(score))
    }

    #[test]
    fn recaptcha_score_valid_boundaries() {
        assert_eq!(
            RecaptchaScore(MIN_SCORE),
            score_from_f64(0.0).expect("min score")
        );
        assert_eq!(
            RecaptchaScore(MID_SCORE),
            score_from_f64(0.5).expect("mid score")
        );
        assert_eq!(
            RecaptchaScore(MAX_SCORE),
            score_from_f64(1.0).expect("max score")
        );
    }

    #[test]
    fn recaptcha_score_invalid_out_of_range() {
        score_from_f64(-0.1).unwrap_err();
        score_from_f64(1.1).unwrap_err();
        score_from_f64(-1.0).unwrap_err();
        score_from_f64(2.0).unwrap_err();
    }

    #[test]
    fn recaptcha_score_invalid_non_finite() {
        score_from_f64(f64::NAN).unwrap_err();
        score_from_f64(f64::INFINITY).unwrap_err();
        score_from_f64(f64::NEG_INFINITY).unwrap_err();
    }

    #[test]
    fn recaptcha_score_visit_f32() {
        assert_eq!(
            RecaptchaScore(MID_SCORE),
            score_from_f32(0.5).expect("mid score")
        );
        score_from_f32(f32::NAN).unwrap_err();
        score_from_f32(1.1).unwrap_err();
    }

    #[test]
    fn recaptcha_score_json_valid() {
        assert_eq!(
            RecaptchaScore(MID_SCORE),
            serde_json::from_str::<RecaptchaScore>("0.5").expect("mid score")
        );
        assert_eq!(
            RecaptchaScore(MIN_SCORE),
            serde_json::from_str::<RecaptchaScore>("0.0").expect("min score")
        );
        assert_eq!(
            RecaptchaScore(MAX_SCORE),
            serde_json::from_str::<RecaptchaScore>("1.0").expect("max score")
        );
    }

    #[test]
    fn recaptcha_score_json_invalid() {
        serde_json::from_str::<RecaptchaScore>("-0.1").unwrap_err();
        serde_json::from_str::<RecaptchaScore>("1.1").unwrap_err();
        serde_json::from_str::<RecaptchaScore>("\"0.5\"").unwrap_err();
        serde_json::from_str::<RecaptchaScore>("null").unwrap_err();
        // JSON integers are deserialized via `visit_u64`/`visit_i64`,
        // which this visitor does not implement, so `1` is rejected
        // even though `1.0` is accepted.
        serde_json::from_str::<RecaptchaScore>("1").unwrap_err();
        serde_json::from_str::<RecaptchaScore>("0").unwrap_err();
    }

    #[test]
    fn recaptcha_score_serde_round_trip() {
        let score = serde_json::from_str::<RecaptchaScore>("0.5").expect("mid score");
        let json = serde_json::to_string(&score).expect("serialize score");
        assert_eq!("0.5", json);
        let round_trip = serde_json::from_str::<RecaptchaScore>(&json).expect("round trip");
        assert_eq!(score, round_trip);
    }
}
