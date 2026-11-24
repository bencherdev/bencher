use bencher_client::types::{Boundary, Confirmations, ModelTest, SampleSize, Significance, Window};
use bencher_json::ModelSlot;

use crate::parser::project::threshold::{
    CliChangepointModel, CliChangepointTest, CliModel, CliModelTest,
};

use super::ThresholdError;

#[derive(Debug, Clone)]
pub struct Model {
    pub test: ModelTest,
    pub min_sample_size: Option<SampleSize>,
    pub max_sample_size: Option<SampleSize>,
    pub window: Option<Window>,
    pub lower_boundary: Option<Boundary>,
    pub upper_boundary: Option<Boundary>,
    pub significance: Option<Significance>,
    pub confirmations: Option<Confirmations>,
}

impl TryFrom<bencher_json::Model> for Model {
    type Error = ThresholdError;

    fn try_from(model: bencher_json::Model) -> Result<Self, Self::Error> {
        model.validate().map_err(ThresholdError::BadModel)?;
        let bencher_json::Model {
            test,
            min_sample_size,
            max_sample_size,
            window,
            lower_boundary,
            upper_boundary,
            significance,
            confirmations,
        } = model;
        Ok(Self {
            test: test.into(),
            min_sample_size: min_sample_size.map(Into::into),
            max_sample_size: max_sample_size.map(Into::into),
            window: window.map(Into::into),
            lower_boundary: lower_boundary.map(Into::into),
            upper_boundary: upper_boundary.map(Into::into),
            significance: significance.map(Into::into),
            confirmations: confirmations.map(Into::into),
        })
    }
}

impl TryFrom<CliModel> for Model {
    type Error = ThresholdError;

    fn try_from(model: CliModel) -> Result<Self, Self::Error> {
        let CliModel {
            test,
            min_sample_size,
            max_sample_size,
            window,
            lower_boundary,
            upper_boundary,
        } = model;
        bencher_json::Model {
            test: test.into(),
            min_sample_size,
            max_sample_size,
            window,
            lower_boundary,
            upper_boundary,
            significance: None,
            confirmations: None,
        }
        .try_into()
    }
}

/// The changepoint model the flags give, if they give one.
pub fn changepoint_model(model: &CliChangepointModel) -> Result<Option<Model>, ThresholdError> {
    let CliChangepointModel {
        test,
        min_sample_size,
        max_sample_size,
        lower_boundary,
        upper_boundary,
        significance,
        confirmations,
    } = *model;
    test.map(|test| {
        bencher_json::Model {
            test: test.into(),
            min_sample_size,
            max_sample_size,
            window: None,
            lower_boundary,
            upper_boundary,
            significance,
            confirmations,
        }
        .try_into()
    })
    .transpose()
}

/// The flat `--test` sets the model, so it refuses a test that belongs in the changepoint model.
pub fn admit_flat_test(test: CliModelTest) -> Result<(), ThresholdError> {
    let test = bencher_json::ModelTest::from(test);
    ModelSlot::Model
        .admit(test)
        .map_err(|_err| ThresholdError::LineTest(test))
}

impl From<CliModelTest> for bencher_json::ModelTest {
    fn from(kind: CliModelTest) -> Self {
        match kind {
            CliModelTest::Static => Self::Static,
            CliModelTest::Percentage => Self::Percentage,
            CliModelTest::ZScore => Self::ZScore,
            CliModelTest::TTest => Self::TTest,
            CliModelTest::LogNormal => Self::LogNormal,
            CliModelTest::Iqr => Self::Iqr,
            CliModelTest::DeltaIqr => Self::DeltaIqr,
            CliModelTest::Changepoint => Self::Changepoint,
        }
    }
}

impl From<CliChangepointTest> for bencher_json::ModelTest {
    fn from(kind: CliChangepointTest) -> Self {
        match kind {
            CliChangepointTest::Changepoint => Self::Changepoint,
        }
    }
}

impl From<Model> for bencher_client::types::Model {
    fn from(model: Model) -> Self {
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
        Self {
            test,
            min_sample_size,
            max_sample_size,
            window,
            lower_boundary,
            upper_boundary,
            significance,
            confirmations,
        }
    }
}
