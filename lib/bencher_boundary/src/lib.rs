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
        Boundary::is_valid_percentage(boundary)
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
pub struct NormalBoundary(OrderedFloat<f64>);

impl TryFrom<f64> for NormalBoundary {
    type Error = BoundaryError;

    fn try_from(boundary: f64) -> Result<Self, Self::Error> {
        // The normal boundary must be greater than or equal to 0.5 and less than 1.0
        Boundary::is_valid_normal(boundary)
            .then(|| Self(boundary.into()))
            .ok_or(BoundaryError::NormalBoundary(boundary))
    }
}

impl From<NormalBoundary> for f64 {
    fn from(boundary: NormalBoundary) -> Self {
        boundary.0.into()
    }
}

impl TryFrom<Boundary> for NormalBoundary {
    type Error = BoundaryError;

    fn try_from(boundary: Boundary) -> Result<Self, Self::Error> {
        f64::from(boundary).try_into()
    }
}

impl From<NormalBoundary> for Boundary {
    fn from(boundary: NormalBoundary) -> Self {
        // This should never fail because Boundary is a superset of NormalBoundary
        f64::from(boundary).try_into().unwrap_or(Boundary::ZERO)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct IqrBoundary(OrderedFloat<f64>);

impl TryFrom<f64> for IqrBoundary {
    type Error = BoundaryError;

    fn try_from(boundary: f64) -> Result<Self, Self::Error> {
        // The inter-quartile range boundary must be greater than or equal to 0.0
        Boundary::is_valid_iqr(boundary)
            .then(|| Self(boundary.into()))
            .ok_or(BoundaryError::IqrBoundary(boundary))
    }
}

impl From<IqrBoundary> for f64 {
    fn from(boundary: IqrBoundary) -> Self {
        boundary.0.into()
    }
}

impl TryFrom<Boundary> for IqrBoundary {
    type Error = BoundaryError;

    fn try_from(boundary: Boundary) -> Result<Self, Self::Error> {
        f64::from(boundary).try_into()
    }
}

impl From<IqrBoundary> for Boundary {
    fn from(boundary: IqrBoundary) -> Self {
        // This should never fail because Boundary is a superset of IqrBoundary
        f64::from(boundary).try_into().unwrap_or(Boundary::ZERO)
    }
}
