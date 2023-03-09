use plotters::prelude::DrawingAreaErrorKind;
use plotters_bitmap::BitMapBackendError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PlotError {
    #[error("Failed to cast integer: {0}")]
    IntError(#[from] std::num::TryFromIntError),
    #[error("Failed to draw plot: {0}")]
    BitMap(#[from] DrawingAreaErrorKind<BitMapBackendError>),
    #[error("Failed to generate image buffer")]
    ImageBuffer,
    #[error("Failed to generate image: {0}")]
    Image(#[from] image::error::ImageError),
}
