use std::convert::TryFrom;

use bencher_json::threshold::{
    JsonNewStatistic,
    JsonStatisticKind,
};

use crate::{
    cli::threshold::{
        CliStatisticCreate,
        CliStatisticKind,
    },
    BencherError,
};

#[derive(Debug, Clone, Copy)]
pub struct Statistic {
    pub kind:        StatisticKind,
    pub sample_size: Option<u32>,
    pub window:      Option<u32>,
    pub left_side:   Option<f32>,
    pub right_side:  Option<f32>,
}

impl TryFrom<CliStatisticCreate> for Statistic {
    type Error = BencherError;

    fn try_from(create: CliStatisticCreate) -> Result<Self, Self::Error> {
        let CliStatisticCreate {
            kind,
            sample_size,
            window,
            left_side,
            right_side,
        } = create;
        Ok(Self {
            kind: kind.into(),
            sample_size,
            window,
            // TODO validate these as reasonable percentages
            left_side,
            right_side,
        })
    }
}

impl Into<JsonNewStatistic> for Statistic {
    fn into(self) -> JsonNewStatistic {
        let Self {
            kind,
            sample_size,
            window,
            left_side,
            right_side,
        } = self;
        JsonNewStatistic {
            kind: kind.into(),
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

impl Into<JsonStatisticKind> for StatisticKind {
    fn into(self) -> JsonStatisticKind {
        match self {
            Self::Z => JsonStatisticKind::Z,
            Self::T => JsonStatisticKind::T,
        }
    }
}
