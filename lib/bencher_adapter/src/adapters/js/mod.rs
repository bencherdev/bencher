pub mod benchmark;
pub mod time;

use crate::{Adaptable, AdapterResults, Settings};
use benchmark::AdapterJsBenchmark;
use time::AdapterJsTime;

pub struct AdapterJs;

impl Adaptable for AdapterJs {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        AdapterJsBenchmark::parse(input, settings).or_else(|| AdapterJsTime::parse(input, settings))
    }
}

#[cfg(test)]
mod test_js {
    use super::{time::test_js_time, AdapterJs};
    use crate::adapters::{js::benchmark::test_js_benchmark, test_util::convert_file_path};

    #[test]
    fn test_adapter_js_benchmark() {
        let results = convert_file_path::<AdapterJs>("./tool_output/js/benchmark/three.txt");
        test_js_benchmark::validate_adapter_js_benchmark(&results);
    }

    #[test]
    fn test_adapter_js_time() {
        let results = convert_file_path::<AdapterJs>("./tool_output/js/time/four.txt");
        test_js_time::validate_adapter_js_time(&results);
    }
}
