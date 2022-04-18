use std::collections::BTreeMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};

pub type Metrics = BTreeMap<String, Metric>;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Metric {
    latency: Option<Latency>,
    throughput: Option<()>,
    total_cpu: Option<()>,
    self_cpu: Option<()>,
    total_memory: Option<()>,
    self_memory: Option<()>,
}

impl From<Latency> for Metric {
    fn from(latency: Latency) -> Self {
        Self {
            latency: Some(latency),
            ..Default::default()
        }
    }
}

impl Metric {
    pub fn latency(&self) -> Option<&Latency> {
        self.latency.as_ref()
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Latency {
    pub duration: Duration,
    pub variance: Duration,
}
