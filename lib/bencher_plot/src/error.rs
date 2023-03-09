use thiserror::Error;

#[derive(Debug, Error)]
pub enum PlotError {
    #[error("Failed to generate plot: {0}")]
    Plotters(String),
    #[error("Failed to cast integer: {0}")]
    IntError(#[from] std::num::TryFromIntError),
    #[error("Failed to generate image buffer")]
    ImageBuffer,
    #[error("Failed to generate image: {0}")]
    Image(#[from] image::error::ImageError),
}
