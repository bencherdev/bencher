use bencher_json::project::report::new::JsonBenchmarksMap;

use crate::AdapterError;

pub trait Convert {
    fn convert(input: &str) -> Result<JsonBenchmarksMap, AdapterError>;
}
