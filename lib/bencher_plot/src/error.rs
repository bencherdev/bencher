use plotters::prelude::{BitMapBackend, DrawingAreaErrorKind};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PlotError {
    #[error("Failed to generate plot: {0}")]
    Plotters(String),
    #[error("Failed to cast integer: {0}")]
    IntError(#[from] std::num::TryFromIntError),
}
