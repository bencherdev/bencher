use std::convert::TryFrom;

use bencher_client::types::JsonStatisticKind;
use bencher_json::{Boundary, SampleSize};

use crate::{
    parser::project::threshold::{CliStatisticCreate, CliStatisticKind},
    CliError,
};

#[derive(Debug, Clone, Copy)]
pub struct Statistic {
    pub test: JsonStatisticKind,
    pub min_sample_size: Option<SampleSize>,
    pub max_sample_size: Option<SampleSize>,
    pub window: Option<u32>,
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
        Ok(Self {
            test: test.into(),
            min_sample_size: map_sample_size(min_sample_size)?,
            max_sample_size: map_sample_size(max_sample_size)?,
            window,
            lower_boundary: map_boundary(lower_boundary)?,
            upper_boundary: map_boundary(upper_boundary)?,
        })
    }
}

fn map_sample_size(sample_size: Option<u32>) -> Result<Option<SampleSize>, CliError> {
    Ok(if let Some(sample_size) = sample_size {
        Some(sample_size.try_into().map_err(CliError::SampleSize)?)
    } else {
        None
    })
}

fn map_boundary(boundary: Option<f64>) -> Result<Option<Boundary>, CliError> {
    Ok(if let Some(boundary) = boundary {
        Some(boundary.try_into().map_err(CliError::Boundary)?)
    } else {
        None
    })
}

impl From<CliStatisticKind> for JsonStatisticKind {
    fn from(kind: CliStatisticKind) -> Self {
        match kind {
            CliStatisticKind::Z => Self::Z,
            CliStatisticKind::T => Self::T,
        }
    }
}
