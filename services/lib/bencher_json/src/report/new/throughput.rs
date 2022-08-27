use std::cmp::Ordering;

use derive_more::{
    Add,
    Sum,
};
use ordered_float::OrderedFloat;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{
    Deserialize,
    Serialize,
};

use super::{
    mean::Mean,
    median::Median,
};

#[derive(Debug, Copy, Clone, Default, Eq, Add, Sum, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonThroughput {
    pub lower_variance: OrderedFloat<f64>,
    pub upper_variance: OrderedFloat<f64>,
    pub events:         OrderedFloat<f64>,
    pub unit_time:      u64,
}

impl PartialEq for JsonThroughput {
    fn eq(&self, other: &Self) -> bool {
        self.lower_variance == other.lower_variance
            && self.upper_variance == other.upper_variance
            && self.events == other.events
            && self.unit_time == other.unit_time
    }
}

impl PartialOrd for JsonThroughput {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for JsonThroughput {
    fn cmp(&self, other: &Self) -> Ordering {
        let events_order = self
            .per_unit_time(&self.events)
            .cmp(&other.per_unit_time(&other.events));
        if Ordering::Equal == events_order {
            let upper_order = self
                .per_unit_time(&self.upper_variance)
                .cmp(&other.per_unit_time(&other.upper_variance));
            if Ordering::Equal == upper_order {
                self.per_unit_time(&self.lower_variance)
                    .cmp(&other.per_unit_time(&other.lower_variance))
            } else {
                upper_order
            }
        } else {
            events_order
        }
    }
}

impl JsonThroughput {
    fn per_unit_time(&self, events: &OrderedFloat<f64>) -> OrderedFloat<f64> {
        OrderedFloat(events.into_inner() / self.unit_time as f64)
    }
}

impl std::ops::Div<usize> for JsonThroughput {
    type Output = Self;

    fn div(self, rhs: usize) -> Self::Output {
        Self {
            lower_variance: self.lower_variance / rhs as f64,
            upper_variance: self.upper_variance / rhs as f64,
            events:         self.events / rhs as f64,
            unit_time:      self.unit_time / rhs as u64,
        }
    }
}

impl Mean for JsonThroughput {}

impl Median for JsonThroughput {}
