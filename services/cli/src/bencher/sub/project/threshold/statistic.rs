use std::convert::TryFrom;

use bencher_json::project::threshold::{JsonNewStatistic, JsonStatisticKind};

use crate::{
    cli::project::threshold::{CliStatisticCreate, CliStatisticKind},
    CliError,
};

#[derive(Debug, Clone, Copy)]
pub struct Statistic {
    pub test: JsonStatisticKind,
    pub min_sample_size: Option<u32>,
    pub max_sample_size: Option<u32>,
    pub window: Option<u32>,
    pub lower_limit: Option<f64>,
    pub upper_limit: Option<f64>,
}

impl TryFrom<CliStatisticCreate> for Statistic {
    type Error = CliError;

    fn try_from(create: CliStatisticCreate) -> Result<Self, Self::Error> {
        let CliStatisticCreate {
            test,
            min_sample_size,
            max_sample_size,
            window,
            lower_limit,
            upper_limit,
        } = create;
        Ok(Self {
            test: test.into(),
            min_sample_size,
            max_sample_size,
            window,
            // TODO validate these as reasonable percentages
            lower_limit,
            upper_limit,
        })
    }
}

impl From<CliStatisticKind> for JsonStatisticKind {
    fn from(kind: CliStatisticKind) -> Self {
        match kind {
            CliStatisticKind::Z => Self::Z,
            CliStatisticKind::T => Self::T,
        }
    }
}

impl From<Statistic> for JsonNewStatistic {
    fn from(statistic: Statistic) -> Self {
        let Statistic {
            test,
            min_sample_size,
            max_sample_size,
            window,
            lower_limit,
            upper_limit,
        } = statistic;
        Self {
            test,
            min_sample_size,
            max_sample_size,
            window,
            lower_limit: lower_limit.map(Into::into),
            upper_limit: upper_limit.map(Into::into),
        }
    }
}
