use std::collections::HashMap;
use std::time::Duration;

use chrono::serde::ts_seconds;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub type Metrics = HashMap<String, Metric>;

#[derive(Debug, Serialize, Deserialize)]
pub struct Report {
    #[serde(with = "ts_seconds")]
    date_time: DateTime<Utc>,
    metrics: Metrics,
}

impl Report {
    pub fn new(metrics: Metrics) -> Self {
        Self {
            date_time: Utc::now(),
            metrics,
        }
    }
}

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
