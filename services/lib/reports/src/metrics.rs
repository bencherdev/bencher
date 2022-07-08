use std::collections::BTreeMap;
use std::time::Duration;

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub type Metrics = BTreeMap<String, Metric>;

#[derive(Debug, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct Metric {
    #[serde(skip_serializing_if = "Option::is_none")]
    latency: Option<Latency>,
    #[serde(skip_serializing_if = "Option::is_none")]
    throughput: Option<()>,
    #[serde(skip_serializing_if = "Option::is_none")]
    total_cpu: Option<()>,
    #[serde(skip_serializing_if = "Option::is_none")]
    self_cpu: Option<()>,
    #[serde(skip_serializing_if = "Option::is_none")]
    total_memory: Option<()>,
    #[serde(skip_serializing_if = "Option::is_none")]
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
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct Latency {
    pub duration: Duration,
    pub variance: Duration,
}
