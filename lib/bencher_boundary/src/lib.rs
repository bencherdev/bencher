pub mod boundary;
mod error;
pub mod limits;
mod ln;
mod mean;
mod quartiles;

pub use boundary::MetricsBoundary;
pub use error::BoundaryError;

#[derive(Debug, Clone)]
pub struct MetricsData {
    pub data: Vec<f64>,
}
