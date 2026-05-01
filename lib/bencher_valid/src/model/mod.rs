#[cfg(feature = "schema")]
use schemars::JsonSchema;
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::wasm_bindgen;

use serde::{Deserialize, Serialize};

use crate::ValidError;

pub mod boundary;
pub mod model_test;
pub mod sample_size;
pub mod window;

use boundary::{Boundary, CdfBoundary, IqrBoundary, PercentageBoundary};
use model_test::ModelTest;
use sample_size::SampleSize;
use window::Window;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct Model {
    /// The test used by the threshold model to calculate the baseline and boundary limits.
    pub test: ModelTest,
    /// The minimum number of historical samples required to perform the test.
    /// If there are fewer historical samples, the test will not be performed.
    /// The new Metric being tested is not counted towards this minimum.
    pub min_sample_size: Option<SampleSize>,
    /// The maximum number of historical samples used to perform the test.
    /// Only the most recent samples will be used if there are more.
    /// The new Metric being tested is not counted towards this maximum.
    pub max_sample_size: Option<SampleSize>,
    /// The window of time for samples used to perform the test, in seconds.
    /// Samples outside of this window will be omitted.
    pub window: Option<Window>,
    /// The lower boundary used to calculate the lower boundary limit.
    /// The requirements for this field depend on which `test` is selected.
    pub lower_boundary: Option<Boundary>,
    /// The upper boundary used to calculate the upper boundary limit.
    /// The requirements for this field depend on which `test` is selected.
    pub upper_boundary: Option<Boundary>,
}

impl Model {
    pub fn lower_boundary() -> Self {
        Self {
            test: ModelTest::TTest,
            min_sample_size: None,
            max_sample_size: Some(SampleSize::SIXTY_FOUR),
            window: None,
            lower_boundary: Some(Boundary::NINETY_NINE),
            upper_boundary: None,
        }
    }

    pub fn upper_boundary() -> Self {
        Self {
            test: ModelTest::TTest,
            min_sample_size: None,
            max_sample_size: Some(SampleSize::SIXTY_FOUR),
            window: None,
            lower_boundary: None,
            upper_boundary: Some(Boundary::NINETY_NINE),
        }
    }

    pub fn validate(self) -> Result<(), ValidError> {
        validate_model(self)
    }
}

pub fn validate_model(model: Model) -> Result<(), ValidError> {
    let Model {
        test,
        min_sample_size,
        max_sample_size,
        window,
        lower_boundary,
        upper_boundary,
    } = model;

    validate_test_sample_size(test, min_sample_size, max_sample_size)?;

    match test {
        ModelTest::Static => {
            if let Some(&window) = window.as_ref() {
                return Err(ValidError::StaticWindow(window));
            }

            match (lower_boundary.as_ref(), upper_boundary.as_ref()) {
                (Some(&lower), Some(&upper)) => {
                    if f64::from(lower) > f64::from(upper) {
                        Err(ValidError::Boundaries { lower, upper })
                    } else {
                        Ok(())
                    }
                },
                (Some(_), None) | (None, Some(_)) => Ok(()),
                (None, None) => Err(ValidError::NoBoundary),
            }
        },
        ModelTest::Percentage => {
            validate_sample_size(min_sample_size, max_sample_size)?;
            validate_boundary::<PercentageBoundary>(lower_boundary, upper_boundary)
        },
        ModelTest::ZScore | ModelTest::TTest | ModelTest::LogNormal => {
            validate_sample_size(min_sample_size, max_sample_size)?;
            validate_boundary::<CdfBoundary>(lower_boundary, upper_boundary)
        },
        ModelTest::Iqr | ModelTest::DeltaIqr => {
            validate_sample_size(min_sample_size, max_sample_size)?;
            validate_boundary::<IqrBoundary>(lower_boundary, upper_boundary)
        },
    }
}

/// Per-test sample-size validation.
///
/// `Static` does not use sample size at all and rejects it outright.
/// `Percentage` only needs a mean, so it accepts a sample size of `1`.
/// All other tests need variance (or quartiles), so they require `>= 2`.
fn validate_test_sample_size(
    test: ModelTest,
    min_sample_size: Option<SampleSize>,
    max_sample_size: Option<SampleSize>,
) -> Result<(), ValidError> {
    let test_min: u32 = match test {
        ModelTest::Static => {
            if let Some(min) = min_sample_size {
                return Err(ValidError::StaticMinSampleSize(min));
            }
            if let Some(max) = max_sample_size {
                return Err(ValidError::StaticMaxSampleSize(max));
            }
            return Ok(());
        },
        ModelTest::Percentage => SampleSize::MIN.into(),
        ModelTest::ZScore
        | ModelTest::TTest
        | ModelTest::LogNormal
        | ModelTest::Iqr
        | ModelTest::DeltaIqr => SampleSize::TWO.into(),
    };
    for sample_size in [min_sample_size, max_sample_size].into_iter().flatten() {
        if u32::from(sample_size) < test_min {
            return Err(ValidError::TestSampleSize {
                test,
                sample_size,
                min: test_min,
            });
        }
    }
    Ok(())
}

fn validate_sample_size(
    min_sample_size: Option<SampleSize>,
    max_sample_size: Option<SampleSize>,
) -> Result<(), ValidError> {
    if let (Some(min), Some(max)) = (min_sample_size, max_sample_size)
        && u32::from(min) > u32::from(max)
    {
        Err(ValidError::SampleSizes { min, max })
    } else {
        Ok(())
    }
}

fn validate_boundary<B>(lower: Option<Boundary>, upper: Option<Boundary>) -> Result<(), ValidError>
where
    B: TryFrom<Boundary, Error = ValidError>,
    f64: From<B>,
{
    match (lower, upper) {
        (Some(lower), Some(upper)) => {
            B::try_from(lower)?;
            B::try_from(upper)?;
            Ok(())
        },
        (Some(boundary), None) | (None, Some(boundary)) => {
            B::try_from(boundary)?;
            Ok(())
        },
        (None, None) => Err(ValidError::NoBoundary),
    }
}

#[cfg(feature = "wasm")]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_model(model: &str) -> bool {
    let Ok(model) = serde_json::from_str(model) else {
        return false;
    };
    validate_model(model).is_ok()
}

#[cfg(test)]
mod tests {
    use crate::{Boundary, ModelTest, SampleSize, ValidError};

    use super::{Model, validate_model, validate_test_sample_size};

    fn percentage_model(
        min_sample_size: Option<SampleSize>,
        max_sample_size: Option<SampleSize>,
    ) -> Model {
        Model {
            test: ModelTest::Percentage,
            min_sample_size,
            max_sample_size,
            window: None,
            lower_boundary: None,
            upper_boundary: Some(Boundary::NINETY_NINE),
        }
    }

    fn t_test_model(
        min_sample_size: Option<SampleSize>,
        max_sample_size: Option<SampleSize>,
    ) -> Model {
        Model {
            test: ModelTest::TTest,
            min_sample_size,
            max_sample_size,
            window: None,
            lower_boundary: None,
            upper_boundary: Some(Boundary::NINETY_NINE),
        }
    }

    #[test]
    fn percentage_accepts_sample_size_of_one() {
        assert!(
            validate_test_sample_size(
                ModelTest::Percentage,
                Some(SampleSize::MIN),
                Some(SampleSize::MIN),
            )
            .is_ok(),
        );
        assert!(validate_model(percentage_model(None, Some(SampleSize::MIN))).is_ok());
        assert!(
            validate_model(percentage_model(
                Some(SampleSize::MIN),
                Some(SampleSize::MIN)
            ))
            .is_ok(),
        );
    }

    #[test]
    fn percentage_accepts_sample_size_of_two() {
        assert!(
            validate_model(percentage_model(
                Some(SampleSize::TWO),
                Some(SampleSize::TWO)
            ))
            .is_ok(),
        );
    }

    #[test]
    fn t_test_rejects_min_sample_size_of_one() {
        let err = validate_model(t_test_model(Some(SampleSize::MIN), None)).unwrap_err();
        assert!(
            matches!(
                err,
                ValidError::TestSampleSize {
                    test: ModelTest::TTest,
                    sample_size,
                    min: 2,
                } if sample_size == SampleSize::MIN
            ),
            "unexpected error: {err:?}",
        );
    }

    #[test]
    fn t_test_rejects_max_sample_size_of_one() {
        let err = validate_model(t_test_model(None, Some(SampleSize::MIN))).unwrap_err();
        assert!(
            matches!(
                err,
                ValidError::TestSampleSize {
                    test: ModelTest::TTest,
                    sample_size,
                    min: 2,
                } if sample_size == SampleSize::MIN
            ),
            "unexpected error: {err:?}",
        );
    }

    #[test]
    fn t_test_accepts_sample_size_of_two() {
        assert!(validate_model(t_test_model(Some(SampleSize::TWO), Some(SampleSize::TWO))).is_ok(),);
    }

    #[test]
    fn iqr_rejects_sample_size_of_one() {
        let err =
            validate_test_sample_size(ModelTest::Iqr, Some(SampleSize::MIN), None).unwrap_err();
        assert!(
            matches!(
                err,
                ValidError::TestSampleSize {
                    test: ModelTest::Iqr,
                    sample_size,
                    min: 2,
                } if sample_size == SampleSize::MIN
            ),
            "unexpected error: {err:?}",
        );
    }

    #[test]
    fn delta_iqr_rejects_sample_size_of_one() {
        let err = validate_test_sample_size(ModelTest::DeltaIqr, None, Some(SampleSize::MIN))
            .unwrap_err();
        assert!(
            matches!(
                err,
                ValidError::TestSampleSize {
                    test: ModelTest::DeltaIqr,
                    sample_size,
                    min: 2,
                } if sample_size == SampleSize::MIN
            ),
            "unexpected error: {err:?}",
        );
    }

    #[test]
    fn z_score_and_log_normal_reject_sample_size_of_one() {
        for test in [ModelTest::ZScore, ModelTest::LogNormal] {
            let err = validate_test_sample_size(test, Some(SampleSize::MIN), None).unwrap_err();
            assert!(
                matches!(
                    err,
                    ValidError::TestSampleSize {
                        sample_size,
                        min: 2,
                        ..
                    } if sample_size == SampleSize::MIN
                ),
                "unexpected error for {test}: {err:?}",
            );
        }
    }

    #[test]
    fn static_still_rejects_any_sample_size() {
        let err =
            validate_test_sample_size(ModelTest::Static, Some(SampleSize::TWO), None).unwrap_err();
        assert!(
            matches!(err, ValidError::StaticMinSampleSize(_)),
            "unexpected error: {err:?}",
        );
        let err =
            validate_test_sample_size(ModelTest::Static, None, Some(SampleSize::TWO)).unwrap_err();
        assert!(
            matches!(err, ValidError::StaticMaxSampleSize(_)),
            "unexpected error: {err:?}",
        );
    }

    #[test]
    fn min_greater_than_max_still_rejected() {
        let err = validate_model(t_test_model(
            Some(SampleSize::SIXTY_FOUR),
            Some(SampleSize::TWO),
        ))
        .unwrap_err();
        assert!(
            matches!(err, ValidError::SampleSizes { .. }),
            "unexpected error: {err:?}",
        );
    }
}
