use std::collections::BTreeMap;
use std::convert::AsMut;
use std::convert::AsRef;

use chrono::{DateTime, Utc};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

#[cfg(feature = "wasm")]
mod data;
mod metrics;

#[cfg(feature = "wasm")]
pub use data::InventoryData;
pub use metrics::{Latency, Metric, Metrics};

#[derive(Debug, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub struct Reports(BTreeMap<DateTime<Utc>, Report>);

impl AsRef<BTreeMap<DateTime<Utc>, Report>> for Reports {
    fn as_ref(&self) -> &BTreeMap<DateTime<Utc>, Report> {
        &self.0
    }
}

impl AsMut<BTreeMap<DateTime<Utc>, Report>> for Reports {
    fn as_mut(&mut self) -> &mut BTreeMap<DateTime<Utc>, Report> {
        &mut self.0
    }
}

impl Reports {
    pub fn add(&mut self, report: Report) {
        self.0.insert(report.date_time, report);
    }
}

impl Reports {
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(feature = "wasm")]
#[wasm_bindgen]
impl Reports {
    pub fn from_str(reports: &str) -> Self {
        Self(serde_json::from_str(reports).expect("Failed to deserialize JSON"))
    }

    pub fn to_string(&self) -> JsValue {
        JsValue::from_serde(&self).expect("Failed to serialize JSON for Reports")
    }

    pub fn latency(&self) -> InventoryData {
        InventoryData::new_latency(&self)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct Report {
    date_time: DateTime<Utc>,
    metrics: Metrics,
}

impl From<Metrics> for Report {
    fn from(metrics: Metrics) -> Self {
        Self {
            date_time: Utc::now(),
            metrics,
        }
    }
}
