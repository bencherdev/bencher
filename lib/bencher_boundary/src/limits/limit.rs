use bencher_json::{Boundary, IqrBoundary, PercentageBoundary};

use crate::quartiles::Quartiles;

#[derive(Debug, PartialEq)]
pub struct MetricsLimit {
    pub value: f64,
}

impl MetricsLimit {
    pub fn percentage_lower(mean: f64, boundary: PercentageBoundary) -> Self {
        Self {
            value: mean - Self::percentage(mean, boundary),
        }
    }

    pub fn percentage_upper(mean: f64, boundary: PercentageBoundary) -> Self {
        Self {
            value: mean + Self::percentage(mean, boundary),
        }
    }

    fn percentage(mean: f64, boundary: PercentageBoundary) -> f64 {
        mean * f64::from(boundary)
    }

    // Flip the absolute limit to the other side of the mean, creating the actual boundary limit.
    pub fn inverse_cdf_lower(mean: f64, abs_limit: f64) -> Self {
        Self {
            value: mean * 2.0 - abs_limit,
        }
    }

    pub fn inverse_cdf_upper(abs_limit: f64) -> Self {
        Self { value: abs_limit }
    }

    pub fn iqr_lower(quartiles: Quartiles, boundary: IqrBoundary) -> Self {
        Self {
            value: quartiles.q2 - quartiles.iqr(boundary),
        }
    }

    pub fn iqr_upper(quartiles: Quartiles, boundary: IqrBoundary) -> Self {
        Self {
            value: quartiles.q2 + quartiles.iqr(boundary),
        }
    }

    pub fn delta_iqr_lower(
        quartiles: Quartiles,
        delta_quartiles: Quartiles,
        boundary: IqrBoundary,
    ) -> Self {
        Self {
            value: quartiles.q2 - Self::delta(quartiles.q2, delta_quartiles, boundary),
        }
    }

    pub fn delta_iqr_upper(
        quartiles: Quartiles,
        delta_quartiles: Quartiles,
        boundary: IqrBoundary,
    ) -> Self {
        Self {
            value: quartiles.q2 + Self::delta(quartiles.q2, delta_quartiles, boundary),
        }
    }

    fn delta(median: f64, delta_quartiles: Quartiles, boundary: IqrBoundary) -> f64 {
        median * delta_quartiles.iqr(boundary)
    }
}

impl From<MetricsLimit> for f64 {
    fn from(limit: MetricsLimit) -> Self {
        limit.value
    }
}

impl From<Boundary> for MetricsLimit {
    fn from(boundary: Boundary) -> Self {
        Self {
            value: boundary.into(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp, clippy::unwrap_used)]
mod test {
    use pretty_assertions::assert_eq;

    use crate::{limits::MetricsLimit, quartiles::Quartiles};

    #[test]
    fn test_limit_percentage_lower() {
        let mean = 1.0;
        let boundary = 0.1.try_into().unwrap();
        let limit = MetricsLimit::percentage_lower(mean, boundary);
        assert_eq!(limit.value, 0.9);
    }

    #[test]
    fn test_limit_percentage_upper() {
        let mean = 1.0;
        let boundary = 0.1.try_into().unwrap();
        let limit = MetricsLimit::percentage_upper(mean, boundary);
        assert_eq!(limit.value, 1.1);
    }

    #[test]
    fn test_limit_inverse_cdf_lower() {
        let mean = 1.0;
        let abs_limit = 2.0;
        let limit = MetricsLimit::inverse_cdf_lower(mean, abs_limit);
        assert_eq!(limit.value, 0.0);
    }

    #[test]
    fn test_limit_inverse_cdf_upper() {
        let abs_limit = 2.0;
        let limit = MetricsLimit::inverse_cdf_upper(abs_limit);
        assert_eq!(limit.value, 2.0);
    }

    #[test]
    fn test_limit_iqr_lower() {
        let quartiles = Quartiles {
            q1: 1.0,
            q2: 2.0,
            q3: 3.0,
        };
        let boundary = 1.5.try_into().unwrap();
        let limit = MetricsLimit::iqr_lower(quartiles, boundary);
        assert_eq!(limit.value, -1.0);
    }

    #[test]
    fn test_limit_iqr_upper() {
        let quartiles = Quartiles {
            q1: 1.0,
            q2: 2.0,
            q3: 3.0,
        };
        let boundary = 1.5.try_into().unwrap();
        let limit = MetricsLimit::iqr_upper(quartiles, boundary);
        assert_eq!(limit.value, 5.0);
    }

    #[test]
    fn test_limit_delta_iqr_lower() {
        let quartiles = Quartiles {
            q1: 1.0,
            q2: 2.0,
            q3: 3.0,
        };
        let delta_quartiles = Quartiles {
            q1: 0.5,
            q2: 1.0,
            q3: 1.5,
        };
        let boundary = 1.5.try_into().unwrap();
        let limit = MetricsLimit::delta_iqr_lower(quartiles, delta_quartiles, boundary);
        assert_eq!(limit.value, -1.0);
    }

    #[test]
    fn test_limit_delta_iqr_upper() {
        let quartiles = Quartiles {
            q1: 1.0,
            q2: 2.0,
            q3: 3.0,
        };
        let delta_quartiles = Quartiles {
            q1: 0.5,
            q2: 1.0,
            q3: 1.5,
        };
        let boundary = 1.5.try_into().unwrap();
        let limit = MetricsLimit::delta_iqr_upper(quartiles, delta_quartiles, boundary);
        assert_eq!(limit.value, 5.0);
    }
}
