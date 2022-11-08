use bencher_json::project::report::new::JsonBenchmarksMap;

use crate::{Adapter, AdapterError};

pub struct AdapterJson;

impl Adapter for AdapterJson {
    fn convert(input: &str) -> Result<JsonBenchmarksMap, AdapterError> {
        serde_json::from_str(input).map_err(AdapterError::Serde)
    }
}
