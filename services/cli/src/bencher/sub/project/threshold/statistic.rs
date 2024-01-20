use bencher_client::types::{Boundary, SampleSize, StatisticKind, Window};
use bencher_json::project::threshold::JsonNewStatistic;

use crate::{
    parser::project::threshold::{CliStatisticCreate, CliStatisticKind},
    CliError,
};

#[derive(Debug, Clone)]
pub struct Statistic {
    pub test: StatisticKind,
    pub min_sample_size: Option<SampleSize>,
    pub max_sample_size: Option<SampleSize>,
    pub window: Option<Window>,
    pub lower_boundary: Option<Boundary>,
    pub upper_boundary: Option<Boundary>,
}

impl TryFrom<CliStatisticCreate> for Statistic {
    type Error = CliError;

    fn try_from(create: CliStatisticCreate) -> Result<Self, Self::Error> {
        let CliStatisticCreate {
            test,
            min_sample_size,
            max_sample_size,
            window,
            lower_boundary,
            upper_boundary,
        } = create;
        let json_statistic = JsonNewStatistic {
            test: test.into(),
            min_sample_size,
            max_sample_size,
            window,
            lower_boundary,
            upper_boundary,
        };
        bencher_boundary::validate_statistic(json_statistic).map_err(CliError::Statistic)?;
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

impl From<CliStatisticKind> for bencher_json::project::threshold::StatisticKind {
    fn from(kind: CliStatisticKind) -> Self {
        match kind {
            CliStatisticKind::Static => Self::Static,
            CliStatisticKind::Percentage => Self::Percentage,
            CliStatisticKind::ZScore => Self::ZScore,
            CliStatisticKind::TTest => Self::TTest,
            CliStatisticKind::LogNormal => Self::LogNormal,
            CliStatisticKind::Iqr => Self::Iqr,
            CliStatisticKind::DeltaIqr => Self::DeltaIqr,
        }
    }
}

impl From<CliStatisticKind> for StatisticKind {
    fn from(kind: CliStatisticKind) -> Self {
        match kind {
            CliStatisticKind::Static => Self::Static,
            CliStatisticKind::Percentage => Self::Percentage,
            CliStatisticKind::ZScore => Self::ZScore,
            CliStatisticKind::TTest => Self::TTest,
            CliStatisticKind::LogNormal => Self::LogNormal,
            CliStatisticKind::Iqr => Self::Iqr,
            CliStatisticKind::DeltaIqr => Self::DeltaIqr,
        }
    }
}
