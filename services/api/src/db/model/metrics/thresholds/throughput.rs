use std::collections::HashMap;

use bencher_json::report::JsonThroughput;

use super::threshold::Threshold;

pub struct Throughput {
    pub report_id:    i32,
    pub threshold:    Threshold,
    pub sample_means: HashMap<String, JsonThroughput>,
}
