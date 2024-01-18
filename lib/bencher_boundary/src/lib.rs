pub mod boundary;
mod error;
pub mod limits;

use bencher_json::Boundary;
pub use boundary::MetricsBoundary;
pub use error::BoundaryError;
use ordered_float::OrderedFloat;

#[derive(Debug, Clone)]
pub struct MetricsData {
    pub data: Vec<f64>,
}

#[derive(Debug, Clone, Copy)]
pub struct PercentageBoundary(OrderedFloat<f64>);

impl TryFrom<f64> for PercentageBoundary {
    type Error = BoundaryError;

    fn try_from(boundary: f64) -> Result<Self, Self::Error> {
        // The percentage boundary must be greater than or equal to 0.0
        (boundary >= 0.0)
            .then(|| Self(boundary.into()))
            .ok_or(BoundaryError::PercentageBoundary(boundary))
    }
}

impl From<PercentageBoundary> for f64 {
    fn from(boundary: PercentageBoundary) -> Self {
        boundary.0.into()
    }
}

impl TryFrom<Boundary> for PercentageBoundary {
    type Error = BoundaryError;

    fn try_from(boundary: Boundary) -> Result<Self, Self::Error> {
        f64::from(boundary).try_into()
    }
}

impl From<PercentageBoundary> for Boundary {
    fn from(boundary: PercentageBoundary) -> Self {
        // This should never fail because Boundary is a superset of PercentageBoundary
        f64::from(boundary).try_into().unwrap_or(Boundary::ZERO)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StatisticalBoundary(OrderedFloat<f64>);

impl TryFrom<f64> for StatisticalBoundary {
    type Error = BoundaryError;

    fn try_from(boundary: f64) -> Result<Self, Self::Error> {
        // The statistical boundary must be greater than or equal to 0.5 and less than 1.0
        if boundary < 0.5 {
            false
        } else {
            boundary < 1.0
        }
        .then(|| Self(boundary.into()))
        .ok_or(BoundaryError::StatisticalBoundary(boundary))
    }
}

impl From<StatisticalBoundary> for f64 {
    fn from(boundary: StatisticalBoundary) -> Self {
        boundary.0.into()
    }
}

impl TryFrom<Boundary> for StatisticalBoundary {
    type Error = BoundaryError;

    fn try_from(boundary: Boundary) -> Result<Self, Self::Error> {
        f64::from(boundary).try_into()
    }
}

impl From<StatisticalBoundary> for Boundary {
    fn from(boundary: StatisticalBoundary) -> Self {
        // This should never fail because Boundary is a superset of StatisticalBoundary
        f64::from(boundary).try_into().unwrap_or(Boundary::ZERO)
    }
}
