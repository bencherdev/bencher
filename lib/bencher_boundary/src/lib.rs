pub mod boundary;
mod error;
pub mod limits;

pub use error::BoundaryError;

#[derive(Debug, Clone)]
pub struct MetricsData {
    pub data: Vec<f64>,
}

#[derive(Debug, Clone, Copy)]
pub enum StatisticKind {
    Z,
    T,
}
