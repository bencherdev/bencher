use std::cmp::Ordering;

use derive_more::{
    Add,
    Sum,
};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{
    Deserialize,
    Serialize,
};

use super::{median::Median, mean::Mean};

#[derive(Debug, Copy, Clone, Default, Eq, Add, Sum, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonLatency {
    pub lower_variance: u64,
    pub upper_variance: u64,
    pub duration:       u64,
}

impl PartialEq for JsonLatency {
    fn eq(&self, other: &Self) -> bool {
        self.lower_variance == other.lower_variance
            && self.upper_variance == other.upper_variance
            && self.duration == other.duration
    }
}

impl PartialOrd for JsonLatency {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for JsonLatency {
    fn cmp(&self, other: &Self) -> Ordering {
        let duration_order = self.duration.cmp(&other.duration);
        if Ordering::Equal == duration_order {
            let upper_order = self.upper_variance.cmp(&other.upper_variance);
            if Ordering::Equal == upper_order {
                self.lower_variance.cmp(&other.lower_variance)
            } else {
                upper_order
            }
        } else {
            duration_order
        }
    }
}

impl std::ops::Div<usize> for JsonLatency {
    type Output = Self;

    fn div(self, rhs: usize) -> Self::Output {
        Self {
            lower_variance: self.lower_variance / rhs as u64,
            upper_variance: self.upper_variance / rhs as u64,
            duration:       self.duration / rhs as u64,
        }
    }
}

impl Mean for JsonLatency {}

impl Median for JsonLatency {}
