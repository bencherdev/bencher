pub mod benchmark;

use crate::{Adapter, AdapterResults};
use benchmark::AdapterJsBenchmark;

pub struct AdapterJs;

impl Adapter for AdapterJs {
    fn parse(input: &str) -> Option<AdapterResults> {
        AdapterJsBenchmark::parse(input)
    }
}

#[cfg(test)]
mod test_js {
    use super::AdapterJs;
    use crate::adapters::{js::benchmark::test_js_benchmark, test_util::convert_file_path};

    #[test]
    fn test_adapter_js_benchmark() {
        let results = convert_file_path::<AdapterJs>("./tool_output/js/benchmark/two.json");
        test_js_benchmark::validate_adapter_js_benchmark(results);
    }
}
