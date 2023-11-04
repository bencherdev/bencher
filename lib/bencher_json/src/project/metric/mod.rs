use std::{cmp::Ordering, collections::HashMap, fmt, iter::Sum, ops::Add};

use bencher_valid::{BenchmarkName, ResourceId};
use ordered_float::OrderedFloat;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

mod mean;
mod median;

pub use mean::Mean;
pub use median::Median;

crate::typed_uuid::typed_uuid!(MetricUuid);

#[typeshare::typeshare]
pub type JsonResultsMap = HashMap<BenchmarkName, JsonMetricsMap>;

#[typeshare::typeshare]
pub type JsonMetricsMap = HashMap<MetricKind, JsonMetric>;

#[typeshare::typeshare]
pub type MetricKind = ResourceId;

#[typeshare::typeshare]
#[derive(Debug, Copy, Clone, Default, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonMetric {
    pub value: OrderedFloat<f64>,
    // TODO remove in due time
    #[serde(alias = "lower_bound")]
    pub lower_value: Option<OrderedFloat<f64>>,
    // TODO remove in due time
    #[serde(alias = "upper_bound")]
    pub upper_value: Option<OrderedFloat<f64>>,
}

impl JsonMetric {
    pub fn new(value: f64, lower_value: Option<f64>, upper_value: Option<f64>) -> Self {
        Self {
            value: OrderedFloat(value),
            lower_value: lower_value.map(OrderedFloat),
            upper_value: upper_value.map(OrderedFloat),
        }
    }
}

impl fmt::Display for JsonMetric {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

impl PartialEq for JsonMetric {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
            && option_eq(self.lower_value, other.lower_value)
            && option_eq(self.upper_value, other.upper_value)
    }
}

fn option_eq<T>(left: Option<T>, right: Option<T>) -> bool
where
    T: PartialEq,
{
    if let Some(left) = left {
        if let Some(right) = right {
            left == right
        } else {
            false
        }
    } else {
        right.is_none()
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
            let upper_order = self.upper_value.cmp(&other.upper_value);
            if Ordering::Equal == upper_order {
                self.lower_value.cmp(&other.lower_value)
            } else {
                upper_order
            }
        } else {
            value_order
        }
    }
}

impl Add for JsonMetric {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        let value = self.value + other.value;
        let lower_value = option_add(self.lower_value, self.value, other.lower_value, other.value);
        let upper_value = option_add(self.upper_value, self.value, other.upper_value, other.value);
        Self {
            value,
            lower_value,
            upper_value,
        }
    }
}

fn option_add<T>(
    left_end: Option<T>,
    left_value: T,
    right_end: Option<T>,
    right_value: T,
) -> Option<T>
where
    T: Add<Output = T>,
{
    if let Some(left_end) = left_end {
        if let Some(right_end) = right_end {
            Some(left_end + right_end)
        } else {
            Some(left_end + right_value)
        }
    } else {
        right_end.map(|re| left_value + re)
    }
}

impl Sum for JsonMetric {
    fn sum<I>(iter: I) -> Self
    where
        I: Iterator<Item = Self>,
    {
        iter.fold(Self::default(), |s, o| s + o)
    }
}

impl std::ops::Div<usize> for JsonMetric {
    type Output = Self;

    #[allow(clippy::cast_precision_loss)]
    fn div(self, rhs: usize) -> Self::Output {
        Self {
            value: self.value / rhs as f64,
            lower_value: self.lower_value.map(|b| b / rhs as f64),
            upper_value: self.upper_value.map(|b| b / rhs as f64),
        }
    }
}

impl Mean for JsonMetric {}

impl Median for JsonMetric {}
