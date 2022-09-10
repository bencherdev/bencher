use std::convert::TryFrom;

use bencher_json::threshold::{JsonNewStatistic, JsonStatisticKind};

use crate::{
    cli::threshold::{CliStatisticCreate, CliStatisticKind},
    BencherError,
};

#[derive(Debug, Clone, Copy)]
pub struct Statistic {
    pub test: StatisticKind,
    pub sample_size: Option<u32>,
    pub window: Option<u32>,
    pub left_side: Option<f32>,
    pub right_side: Option<f32>,
}

impl TryFrom<CliStatisticCreate> for Statistic {
    type Error = BencherError;

    fn try_from(create: CliStatisticCreate) -> Result<Self, Self::Error> {
        let CliStatisticCreate {
            test,
            sample_size,
            window,
            left_side,
            right_side,
        } = create;
        Ok(Self {
            test: test.into(),
            sample_size,
            window,
            // TODO validate these as reasonable percentages
            left_side,
            right_side,
        })
    }
}

impl From<Statistic> for JsonNewStatistic {
    fn from(statistic: Statistic) -> Self {
        let Statistic {
            test,
            sample_size,
            window,
            left_side,
            right_side,
        } = statistic;
        Self {
            test: test.into(),
            sample_size,
            window,
            left_side: left_side.map(|s| s.into()),
            right_side: right_side.map(|s| s.into()),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum StatisticKind {
    Z,
    T,
}

impl From<CliStatisticKind> for StatisticKind {
    fn from(kind: CliStatisticKind) -> Self {
        match kind {
            CliStatisticKind::Z => Self::Z,
            CliStatisticKind::T => Self::T,
        }
    }
}

impl From<StatisticKind> for JsonStatisticKind {
    fn from(kind: StatisticKind) -> Self {
        match kind {
            StatisticKind::Z => Self::Z,
            StatisticKind::T => Self::T,
        }
    }
}
