#[cfg(feature = "schema")]
use schemars::JsonSchema;
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::wasm_bindgen;

use serde::{Deserialize, Serialize};

use crate::ValidError;

pub mod boundary;
pub mod confirmations;
pub mod model_test;
pub mod sample_size;
pub mod significance;
pub mod window;

use boundary::{Boundary, CdfBoundary, IqrBoundary, PercentageBoundary};
use confirmations::Confirmations;
use model_test::ModelTest;
use sample_size::SampleSize;
use significance::Significance;
use window::Window;

/// The most points a changepoint model's window may hold.
const CHANGEPOINT_MAX_WINDOW: u32 = 1000;

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
    /// The chance of a false alarm the test may take, for example `0.001`.
    /// Required for the `changepoint` test, where it is the largest p value of a changepoint
    /// on each scan, and refused for every other test.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub significance: Option<Significance>,
    /// The number of consecutive scans that must find a changepoint before it is confirmed,
    /// from 1 to 20.
    /// Optional for the `changepoint` test, which stores 5 when it is not set, and refused for
    /// every other test.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirmations: Option<Confirmations>,
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
            significance: None,
            confirmations: None,
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
            significance: None,
            confirmations: None,
        }
    }

    pub fn validate(self) -> Result<(), ValidError> {
        validate_model(self)
    }

    /// The model as it is stored, with the count of confirming scans its test takes when it
    /// names none.
    #[must_use]
    pub fn with_defaults(self) -> Self {
        let confirmations = self.confirmations.or_else(|| {
            self.test
                .takes_confirmations()
                .then_some(Confirmations::DEFAULT)
        });
        Self {
            confirmations,
            ..self
        }
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
        significance,
        confirmations,
    } = model;

    validate_significance(test, significance)?;
    if confirmations.is_some() && !test.takes_confirmations() {
        return Err(ValidError::ModelFieldRefused {
            test,
            field: "confirmations",
        });
    }
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
        ModelTest::Changepoint => validate_changepoint(
            min_sample_size,
            max_sample_size,
            window,
            lower_boundary,
            upper_boundary,
        ),
    }
}

/// A test that takes a significance requires one, and every other test refuses it.
fn validate_significance(
    test: ModelTest,
    significance: Option<Significance>,
) -> Result<(), ValidError> {
    let takes = test.takes_significance();
    if takes && significance.is_none() {
        Err(ValidError::ModelFieldRequired {
            test,
            field: "significance",
        })
    } else if !takes && significance.is_some() {
        Err(ValidError::ModelFieldRefused {
            test,
            field: "significance",
        })
    } else {
        Ok(())
    }
}

/// A changepoint model's window is its maximum sample size, counted in points, so it has no
/// time window, and its boundaries are relative changes.
fn validate_changepoint(
    min_sample_size: Option<SampleSize>,
    max_sample_size: Option<SampleSize>,
    window: Option<Window>,
    lower_boundary: Option<Boundary>,
    upper_boundary: Option<Boundary>,
) -> Result<(), ValidError> {
    const TEST: ModelTest = ModelTest::Changepoint;
    if window.is_some() {
        return Err(ValidError::ModelFieldRefused {
            test: TEST,
            field: "window",
        });
    }
    let Some(max_sample_size) = max_sample_size else {
        return Err(ValidError::ModelFieldRequired {
            test: TEST,
            field: "max_sample_size",
        });
    };
    let max = u32::from(max_sample_size);
    if !(u32::from(SampleSize::TWO)..=CHANGEPOINT_MAX_WINDOW).contains(&max) {
        return Err(ValidError::ModelFieldRange {
            test: TEST,
            field: "max_sample_size",
            value: f64::from(max),
            expected: "from 2 to 1000",
        });
    }
    if let Some(min_sample_size) = min_sample_size {
        let min = u32::from(min_sample_size);
        if !(u32::from(SampleSize::TWO)..=max).contains(&min) {
            return Err(ValidError::ModelFieldRange {
                test: TEST,
                field: "min_sample_size",
                value: f64::from(min),
                expected: "at least 2 and at most `max_sample_size`",
            });
        }
    }
    if lower_boundary.is_none() && upper_boundary.is_none() {
        return Err(ValidError::ModelBoundaryRequired { test: TEST });
    }
    for (field, boundary) in [
        ("lower_boundary", lower_boundary),
        ("upper_boundary", upper_boundary),
    ] {
        if let Some(boundary) = boundary
            && PercentageBoundary::try_from(boundary).is_err()
        {
            return Err(ValidError::ModelFieldRange {
                test: TEST,
                field,
                value: f64::from(boundary),
                expected: "a relative change of at least 0",
            });
        }
    }
    Ok(())
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
    let test_min: SampleSize = match test {
        ModelTest::Static => {
            if let Some(min) = min_sample_size {
                return Err(ValidError::StaticMinSampleSize(min));
            }
            if let Some(max) = max_sample_size {
                return Err(ValidError::StaticMaxSampleSize(max));
            }
            return Ok(());
        },
        // Its own rules name each field.
        ModelTest::Changepoint => return Ok(()),
        ModelTest::Percentage => SampleSize::MIN,
        ModelTest::ZScore
        | ModelTest::TTest
        | ModelTest::LogNormal
        | ModelTest::Iqr
        | ModelTest::DeltaIqr => SampleSize::TWO,
    };
    for sample_size in [min_sample_size, max_sample_size].into_iter().flatten() {
        if sample_size < test_min {
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
    use crate::{Boundary, Confirmations, ModelTest, SampleSize, Significance, ValidError, Window};

    use super::{Model, validate_model, validate_test_sample_size};

    fn changepoint_model() -> Model {
        Model {
            test: ModelTest::Changepoint,
            min_sample_size: None,
            max_sample_size: Some(sample_size(50)),
            window: None,
            lower_boundary: None,
            upper_boundary: Some(boundary(0.05)),
            significance: Some(Significance::try_from(0.001).unwrap()),
            confirmations: None,
        }
    }

    fn sample_size(sample_size: u32) -> SampleSize {
        SampleSize::try_from(sample_size).unwrap()
    }

    fn boundary(boundary: f64) -> Boundary {
        Boundary::try_from(boundary).unwrap()
    }

    #[track_caller]
    fn assert_names(model: Model, field: &str) {
        let err = validate_model(model).unwrap_err().to_string();
        assert!(err.contains(&format!("`{field}`")), "{field}: {err}");
    }

    #[test]
    fn changepoint_accepts_a_window_a_boundary_and_a_significance() {
        validate_model(changepoint_model()).unwrap();
        validate_model(Model {
            min_sample_size: Some(SampleSize::TWO),
            lower_boundary: Some(boundary(0.1)),
            upper_boundary: None,
            ..changepoint_model()
        })
        .unwrap();
        validate_model(Model {
            min_sample_size: Some(sample_size(50)),
            lower_boundary: Some(Boundary::ZERO),
            ..changepoint_model()
        })
        .unwrap();
    }

    #[test]
    fn changepoint_requires_max_sample_size() {
        assert_names(
            Model {
                max_sample_size: None,
                ..changepoint_model()
            },
            "max_sample_size",
        );
    }

    #[test]
    fn changepoint_max_sample_size_is_from_2_to_1000() {
        for valid in [2, 1000] {
            validate_model(Model {
                max_sample_size: Some(sample_size(valid)),
                ..changepoint_model()
            })
            .unwrap();
        }
        for invalid in [1, 1001] {
            assert_names(
                Model {
                    max_sample_size: Some(sample_size(invalid)),
                    ..changepoint_model()
                },
                "max_sample_size",
            );
        }
    }

    #[test]
    fn changepoint_min_sample_size_is_at_least_2_and_at_most_max_sample_size() {
        for invalid in [1, 51] {
            assert_names(
                Model {
                    min_sample_size: Some(sample_size(invalid)),
                    ..changepoint_model()
                },
                "min_sample_size",
            );
        }
    }

    #[test]
    fn changepoint_requires_a_boundary() {
        let model = Model {
            lower_boundary: None,
            upper_boundary: None,
            ..changepoint_model()
        };
        assert_names(model, "lower_boundary");
        assert_names(model, "upper_boundary");
    }

    #[test]
    fn changepoint_boundaries_are_percentage_boundaries() {
        assert_names(
            Model {
                lower_boundary: Some(boundary(-0.05)),
                ..changepoint_model()
            },
            "lower_boundary",
        );
        assert_names(
            Model {
                upper_boundary: Some(boundary(-0.05)),
                ..changepoint_model()
            },
            "upper_boundary",
        );
    }

    #[test]
    fn changepoint_requires_significance() {
        assert_names(
            Model {
                significance: None,
                ..changepoint_model()
            },
            "significance",
        );
    }

    #[test]
    fn changepoint_refuses_window() {
        assert_names(
            Model {
                window: Some(Window::try_from(86_400).unwrap()),
                ..changepoint_model()
            },
            "window",
        );
    }

    #[test]
    fn every_other_test_refuses_significance_and_confirmations() {
        let significance = Some(Significance::try_from(0.001).unwrap());
        for (test, upper_boundary, max_sample_size) in [
            (ModelTest::Static, 100.0, None),
            (ModelTest::Percentage, 0.1, Some(SampleSize::THIRTY)),
            (ModelTest::ZScore, 0.99, Some(SampleSize::THIRTY)),
            (ModelTest::TTest, 0.99, Some(SampleSize::THIRTY)),
            (ModelTest::LogNormal, 0.99, Some(SampleSize::THIRTY)),
            (ModelTest::Iqr, 3.0, Some(SampleSize::THIRTY)),
            (ModelTest::DeltaIqr, 3.0, Some(SampleSize::THIRTY)),
        ] {
            let model = Model {
                test,
                min_sample_size: None,
                max_sample_size,
                window: None,
                lower_boundary: None,
                upper_boundary: Some(boundary(upper_boundary)),
                significance: None,
                confirmations: None,
            };
            validate_model(model).unwrap();
            assert_names(
                Model {
                    significance,
                    ..model
                },
                "significance",
            );
            assert_names(
                Model {
                    confirmations: Some(Confirmations::DEFAULT),
                    ..model
                },
                "confirmations",
            );
            assert_eq!(model, model.with_defaults(), "{test}");
        }
    }

    #[test]
    fn changepoint_takes_confirmations() {
        validate_model(Model {
            confirmations: Some(Confirmations::try_from(20).unwrap()),
            ..changepoint_model()
        })
        .unwrap();
    }

    #[test]
    fn changepoint_stores_5_confirmations_when_it_names_none() {
        assert_eq!(
            Some(Confirmations::try_from(5).unwrap()),
            changepoint_model().with_defaults().confirmations
        );
        let named = Model {
            confirmations: Some(Confirmations::try_from(2).unwrap()),
            ..changepoint_model()
        };
        assert_eq!(named, named.with_defaults());
    }

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
            significance: None,
            confirmations: None,
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
            significance: None,
            confirmations: None,
        }
    }

    #[test]
    fn percentage_accepts_sample_size_of_one() {
        validate_test_sample_size(
            ModelTest::Percentage,
            Some(SampleSize::MIN),
            Some(SampleSize::MIN),
        )
        .unwrap();
        validate_model(percentage_model(None, Some(SampleSize::MIN))).unwrap();
        validate_model(percentage_model(
            Some(SampleSize::MIN),
            Some(SampleSize::MIN),
        ))
        .unwrap();
    }

    #[test]
    fn percentage_accepts_sample_size_of_two() {
        validate_model(percentage_model(
            Some(SampleSize::TWO),
            Some(SampleSize::TWO),
        ))
        .unwrap();
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
                    min: SampleSize::TWO,
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
                    min: SampleSize::TWO,
                } if sample_size == SampleSize::MIN
            ),
            "unexpected error: {err:?}",
        );
    }

    #[test]
    fn t_test_accepts_sample_size_of_two() {
        validate_model(t_test_model(Some(SampleSize::TWO), Some(SampleSize::TWO))).unwrap();
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
                    min: SampleSize::TWO,
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
                    min: SampleSize::TWO,
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
                        min: SampleSize::TWO,
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
