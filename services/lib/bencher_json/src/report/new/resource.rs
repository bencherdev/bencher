use std::cmp::Ordering;

use derive_more::{Add, Sum};
use ordered_float::OrderedFloat;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{mean::Mean, median::Median};

#[derive(Debug, Copy, Clone, Default, Eq, Add, Sum, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonResource {
    pub min: OrderedFloat<f64>,
    pub max: OrderedFloat<f64>,
    pub avg: OrderedFloat<f64>,
}

impl PartialEq for JsonResource {
    fn eq(&self, other: &Self) -> bool {
        self.min == other.min && self.max == other.max && self.avg == other.avg
    }
}

impl PartialOrd for JsonResource {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for JsonResource {
    fn cmp(&self, other: &Self) -> Ordering {
        let avg_order = self.avg.cmp(&other.avg);
        if Ordering::Equal == avg_order {
            let max_order = self.max.cmp(&other.max);
            if Ordering::Equal == max_order {
                self.min.cmp(&other.min)
            } else {
                max_order
            }
        } else {
            avg_order
        }
    }
}

impl std::ops::Div<usize> for JsonResource {
    type Output = Self;

    fn div(self, rhs: usize) -> Self::Output {
        Self {
            min: self.min / rhs as f64,
            max: self.max / rhs as f64,
            avg: self.avg / rhs as f64,
        }
    }
}

impl Mean for JsonResource {}

impl Median for JsonResource {}
