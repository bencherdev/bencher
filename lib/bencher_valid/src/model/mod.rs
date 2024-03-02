#[cfg(feature = "schema")]
use schemars::JsonSchema;
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

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

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct Model {
    /// The test used by the threshold model to calculate the baseline and boundary limits.
    pub test: ModelTest,
    /// The minimum number of samples required to perform the test.
    /// If there are fewer samples, the test will not be performed.
    pub min_sample_size: Option<SampleSize>,
    /// The maximum number of samples used to perform the test.
    /// Only the most recent samples will be used if there are more.
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
            max_sample_size: Some(SampleSize::TWO_FIFTY_FIVE),
            window: None,
            lower_boundary: Some(Boundary::NINETY_EIGHT),
            upper_boundary: None,
        }
    }

    pub fn upper_boundary() -> Self {
        Self {
            test: ModelTest::TTest,
            min_sample_size: None,
            max_sample_size: Some(SampleSize::TWO_FIFTY_FIVE),
            window: None,
            lower_boundary: None,
            upper_boundary: Some(Boundary::NINETY_EIGHT),
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
    match test {
        ModelTest::Static => {
            if let Some(&min_sample_size) = min_sample_size.as_ref() {
                return Err(ValidError::StaticMinSampleSize(min_sample_size));
            } else if let Some(&max_sample_size) = max_sample_size.as_ref() {
                return Err(ValidError::StaticMaxSampleSize(max_sample_size));
            } else if let Some(&window) = window.as_ref() {
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

fn validate_sample_size(
    min_sample_size: Option<SampleSize>,
    max_sample_size: Option<SampleSize>,
) -> Result<(), ValidError> {
    if let (Some(min), Some(max)) = (min_sample_size, max_sample_size) {
        if u32::from(min) > u32::from(max) {
            return Err(ValidError::SampleSizes { min, max });
        }
    }

    Ok(())
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
