use std::{cmp::Ordering, collections::HashMap, fmt, iter::Sum, ops::Add};

use bencher_valid::{BenchmarkName, NameId};
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
pub type JsonMetricsMap = HashMap<MeasureNameId, JsonNewMetric>;

#[typeshare::typeshare]
pub type MeasureNameId = NameId;

#[typeshare::typeshare]
#[derive(Debug, Copy, Clone, Default, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewMetric {
    pub value: OrderedFloat<f64>,
    pub lower_value: Option<OrderedFloat<f64>>,
    pub upper_value: Option<OrderedFloat<f64>>,
}

#[typeshare::typeshare]
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonMetric {
    pub uuid: MetricUuid,
    pub value: OrderedFloat<f64>,
    pub lower_value: Option<OrderedFloat<f64>>,
    pub upper_value: Option<OrderedFloat<f64>>,
}

impl JsonNewMetric {
    pub fn results(results: Vec<(BenchmarkName, Vec<(MeasureNameId, Self)>)>) -> JsonResultsMap {
        results
            .into_iter()
            .map(|(benchmark_name, measure_metrics)| {
                (benchmark_name, measure_metrics.into_iter().collect())
            })
            .collect()
    }
}

impl PartialEq for JsonNewMetric {
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

impl PartialOrd for JsonNewMetric {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for JsonNewMetric {
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

impl Add for JsonNewMetric {
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

impl Sum for JsonNewMetric {
    fn sum<I>(iter: I) -> Self
    where
        I: Iterator<Item = Self>,
    {
        iter.fold(Self::default(), |s, o| s + o)
    }
}

impl std::ops::Div<usize> for JsonNewMetric {
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

impl Mean for JsonNewMetric {}

impl Median for JsonNewMetric {}

impl fmt::Display for JsonMetric {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}
