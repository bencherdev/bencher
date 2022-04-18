use std::collections::BTreeMap;
use std::convert::AsMut;
use std::convert::AsRef;
use std::string::ToString;
use std::time::Duration;

use chrono::serde::ts_seconds;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

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

#[wasm_bindgen]
impl Reports {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    pub fn add(&mut self, report: Report) {
        self.0.insert(*report.date_time(), report);
    }

    pub fn from_str(reports: &str) -> Self {
        Self(serde_json::from_str(reports).expect("Failed to deserialize JSON"))
    }

    pub fn to_string(&self) -> String {
        serde_json::to_string(&self).expect("Failed to serialize JSON")
    }
}

#[wasm_bindgen]
#[derive(Debug, Serialize, Deserialize)]
pub struct Report {
    #[serde(with = "ts_seconds")]
    date_time: DateTime<Utc>,
    metrics: Metrics,
}

pub type Metrics = BTreeMap<String, Metric>;

impl From<Metrics> for Report {
    fn from(metrics: Metrics) -> Self {
        Self {
            date_time: Utc::now(),
            metrics,
        }
    }
}

impl Report {
    pub fn date_time(&self) -> &DateTime<Utc> {
        &self.date_time
    }
}

#[wasm_bindgen]
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Metric {
    latency: Option<Latency>,
    throughput: Option<()>,
    total_cpu: Option<()>,
    self_cpu: Option<()>,
    total_memory: Option<()>,
    self_memory: Option<()>,
}

impl Metric {
    pub fn from_lateny(latency: Latency) -> Self {
        Self {
            latency: Some(latency),
            ..Default::default()
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Latency {
    pub duration: Duration,
    pub variance: Duration,
}
