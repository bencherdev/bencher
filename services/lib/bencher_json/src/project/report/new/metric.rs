use std::cmp::Ordering;

use derive_more::{Add, Sum};
use ordered_float::OrderedFloat;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{mean::Mean, median::Median};

#[derive(Debug, Copy, Clone, Default, Eq, Add, Sum, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonMetric {
    pub lower_variance: OrderedFloat<f64>,
    pub upper_variance: OrderedFloat<f64>,
    pub value: OrderedFloat<f64>,
}

impl PartialEq for JsonMetric {
    fn eq(&self, other: &Self) -> bool {
        self.lower_variance == other.lower_variance
            && self.upper_variance == other.upper_variance
            && self.value == other.value
    }
}

impl PartialOrd for JsonMetric {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for JsonMetric {
    fn cmp(&self, other: &Self) -> Ordering {
        let value_order = self.value.cmp(&other.value);
        if Ordering::Equal == value_order {
            let upper_order = self.upper_variance.cmp(&other.upper_variance);
            if Ordering::Equal == upper_order {
                self.lower_variance.cmp(&other.lower_variance)
            } else {
                upper_order
            }
        } else {
            value_order
        }
    }
}

impl std::ops::Div<usize> for JsonMetric {
    type Output = Self;

    fn div(self, rhs: usize) -> Self::Output {
        Self {
            lower_variance: self.lower_variance / rhs as f64,
            upper_variance: self.upper_variance / rhs as f64,
            value: self.value / rhs as f64,
        }
    }
}

impl Mean for JsonMetric {}

impl Median for JsonMetric {}
