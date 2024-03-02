use bencher_client::types::{Boundary, ModelTest, SampleSize, Window};

use crate::{
    parser::project::threshold::{CliModel, CliModelTest},
    CliError,
};

#[derive(Debug, Clone)]
pub struct Model {
    pub test: ModelTest,
    pub min_sample_size: Option<SampleSize>,
    pub max_sample_size: Option<SampleSize>,
    pub window: Option<Window>,
    pub lower_boundary: Option<Boundary>,
    pub upper_boundary: Option<Boundary>,
}

impl TryFrom<CliModel> for Model {
    type Error = CliError;

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
        }
        .validate()
        .map_err(CliError::Model)?;
        Ok(Self {
            test: test.into(),
            min_sample_size: min_sample_size.map(Into::into),
            max_sample_size: max_sample_size.map(Into::into),
            window: window.map(Into::into),
            lower_boundary: lower_boundary.map(Into::into),
            upper_boundary: upper_boundary.map(Into::into),
        })
    }
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
        }
    }
}

impl From<CliModelTest> for ModelTest {
    fn from(kind: CliModelTest) -> Self {
        match kind {
            CliModelTest::Static => Self::Static,
            CliModelTest::Percentage => Self::Percentage,
            CliModelTest::ZScore => Self::ZScore,
            CliModelTest::TTest => Self::TTest,
            CliModelTest::LogNormal => Self::LogNormal,
            CliModelTest::Iqr => Self::Iqr,
            CliModelTest::DeltaIqr => Self::DeltaIqr,
        }
    }
}
