use std::convert::TryFrom;

use bencher_client::types::JsonStatisticKind;
use bencher_json::Boundary;

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
            min_sample_size,
            max_sample_size,
            window,
            lower_boundary: map_boundary(lower_boundary)?,
            upper_boundary: map_boundary(upper_boundary)?,
        })
    }
}

fn map_boundary(boundary: Option<f64>) -> Result<Option<Boundary>, CliError> {
    Ok(if let Some(boundary) = boundary {
        Some(boundary.try_into()?)
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
