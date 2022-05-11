use std::collections::BTreeMap;
use std::convert::AsMut;
use std::convert::AsRef;

use chrono::{DateTime, Utc};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

mod data;
mod metrics;
mod testbed;

pub use data::Data;
#[cfg(feature = "wasm")]
pub use data::InventoryData;
pub use metrics::{Latency, Metric, Metrics};
pub use testbed::Testbed;

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
    pub email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    pub testbed: Testbed,
    pub date_time: DateTime<Utc>,
    pub metrics: Metrics,
}

impl Report {
    pub fn new(email: String, project: Option<String>, testbed: Testbed, metrics: Metrics) -> Self {
        Self {
            email,
            project,
            testbed,
            date_time: Utc::now(),
            metrics,
        }
    }
}
