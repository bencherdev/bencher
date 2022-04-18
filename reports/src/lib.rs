use std::collections::BTreeMap;
use std::convert::AsMut;
use std::convert::AsRef;

use chrono::{serde::ts_seconds, DateTime, Utc};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

mod data;
mod metrics;
mod utils;

pub use data::InventoryData;
pub use metrics::{Latency, Metric, Metrics};

#[wasm_bindgen]
#[derive(Debug, Serialize, Deserialize)]
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
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    pub fn add(&mut self, report: Report) {
        self.0.insert(report.date_time, report);
    }
}

#[wasm_bindgen]
impl Reports {
    pub fn from_str(reports: &str) -> Self {
        utils::set_panic_hook();
        Self(serde_json::from_str(reports).expect("Failed to deserialize JSON"))
    }

    pub fn to_string(&self) -> String {
        serde_json::to_string(&self).expect("Failed to serialize JSON for Reports")
    }

    pub fn latency(&self) -> InventoryData {
        InventoryData::new_latency(&self)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Report {
    #[serde(with = "ts_seconds")]
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
