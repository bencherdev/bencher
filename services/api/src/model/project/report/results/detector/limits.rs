use statrs::distribution::{ContinuousCDF, Normal, StudentsT};

use crate::{error::api_error, model::project::threshold::alert::Side, ApiError};

#[derive(Default)]
pub struct Limits {
    pub left: Option<Limit>,
    pub right: Option<Limit>,
}

pub struct Limit {
    pub value: f64,
}

pub enum TestKind {
    Z,
    T { freedom: f64 },
}

impl Limits {
    pub fn new(
        mean: f64,
        std_dev: f64,
        test_kind: TestKind,
        left_side: Option<f64>,
        right_side: Option<f64>,
    ) -> Result<Self, ApiError> {
        Ok(match test_kind {
            // Create a normal distribution and calculate the boundary limits for the threshold based on the boundary percentiles.
            TestKind::Z => {
                let normal = Normal::new(mean, std_dev).map_err(api_error!())?;
                let left = left_side.map(|limit| {
                    let abs_limit = normal.inverse_cdf(limit);
                    Limit::left(mean, abs_limit)
                });
                let right = right_side.map(|limit| {
                    let abs_limit = normal.inverse_cdf(limit);
                    Limit::right(abs_limit)
                });
                Self { left, right }
            },
            // Create a Student's t distribution and calculate the boundary limits for the threshold based on the boundary percentiles.
            TestKind::T { freedom } => {
                let students_t = StudentsT::new(mean, std_dev, freedom).map_err(api_error!())?;
                let left = left_side.map(|limit| {
                    let abs_limit = students_t.inverse_cdf(limit);
                    Limit::left(mean, abs_limit)
                });
                let right = right_side.map(|limit| {
                    let abs_limit = students_t.inverse_cdf(limit);
                    Limit::right(abs_limit)
                });
                Self { left, right }
            },
        })
    }

    // An outlier occurs when the  datum exceeds a boundary limit.
    pub fn outlier(&self, datum: f64) -> Option<Side> {
        match (self.left.as_ref(), self.right.as_ref()) {
            (Some(left), Some(right)) => {
                if datum < left.value {
                    Some(Side::Left)
                } else if datum > right.value {
                    Some(Side::Right)
                } else {
                    None
                }
            },
            (Some(left), None) => (datum < left.value).then_some(Side::Left),
            (None, Some(right)) => (datum > right.value).then_some(Side::Right),
            (None, None) => None,
        }
    }
}

impl Limit {
    // Flip the absolute limit to the other side of the mean, creating the actual boundary limit.
    fn left(mean: f64, abs_limit: f64) -> Self {
        Self {
            value: mean * 2.0 - abs_limit,
        }
    }

    fn right(abs_limit: f64) -> Self {
        Self { value: abs_limit }
    }
}

impl From<Limit> for f64 {
    fn from(limit: Limit) -> Self {
        limit.value
    }
}

#[cfg(test)]
mod test {
    use pretty_assertions::assert_eq;
    use statrs::distribution::{ContinuousCDF, Normal, StudentsT};

    #[test]
    fn test_outlier() {
        let normal = Normal::new(1.0, 1.0).unwrap();
        let percentile = normal.cdf(2.0);
        assert_eq!(percentile, 0.8413447460549428);

        let inverse = normal.inverse_cdf(0.6);
        assert_eq!(inverse, 1.2533471031357997);

        let inverse = normal.inverse_cdf(0.8413447460549428);
        assert_eq!(inverse, 1.999999999943794);

        let students_t = StudentsT::new(1.0, 1.0, 10.0).unwrap();
        let percentile = students_t.cdf(2.0);
        assert_eq!(percentile, 0.8295534338489701);

        let inverse = students_t.inverse_cdf(0.8295534338489701);
        assert_eq!(inverse, 2.000000000000001);
    }
}
