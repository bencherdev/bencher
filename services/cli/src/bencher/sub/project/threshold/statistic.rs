use bencher_client::types::{Boundary, SampleSize, StatisticKind, Window};

use crate::parser::project::threshold::{CliStatisticCreate, CliStatisticKind};

#[derive(Debug, Clone)]
pub struct Statistic {
    pub test: StatisticKind,
    pub min_sample_size: Option<SampleSize>,
    pub max_sample_size: Option<SampleSize>,
    pub window: Option<Window>,
    pub lower_boundary: Option<Boundary>,
    pub upper_boundary: Option<Boundary>,
}

impl From<CliStatisticCreate> for Statistic {
    fn from(create: CliStatisticCreate) -> Self {
        let CliStatisticCreate {
            test,
            min_sample_size,
            max_sample_size,
            window,
            lower_boundary,
            upper_boundary,
        } = create;
        Self {
            test: test.into(),
            min_sample_size: min_sample_size.map(Into::into),
            max_sample_size: max_sample_size.map(Into::into),
            window: window.map(Into::into),
            lower_boundary: lower_boundary.map(Into::into),
            upper_boundary: upper_boundary.map(Into::into),
        }
    }
}

impl From<CliStatisticKind> for StatisticKind {
    fn from(kind: CliStatisticKind) -> Self {
        match kind {
            CliStatisticKind::Z => Self::ZScore,
            CliStatisticKind::T => Self::TTest,
        }
    }
}
