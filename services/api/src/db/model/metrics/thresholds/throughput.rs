use std::collections::HashMap;

use bencher_json::report::JsonThroughput;

use super::threshold::Threshold;

const PERF_ERROR: &str = "Failed to create perf statistic.";

pub struct Throughput {
    pub threshold:    Threshold,
    pub sample_means: HashMap<String, JsonThroughput>,
}
