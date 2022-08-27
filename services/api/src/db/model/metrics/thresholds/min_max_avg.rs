use std::collections::HashMap;

use bencher_json::report::JsonMinMaxAvg;

use super::Threshold;


pub struct MinMaxAvg {
    pub threshold:    Threshold,
    pub sample_means: HashMap<String, JsonMinMaxAvg>,
}