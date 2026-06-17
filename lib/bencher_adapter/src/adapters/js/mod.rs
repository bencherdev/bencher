pub mod benchmark;
pub mod time;
pub mod vitest;

use crate::{Adaptable, AdapterResults, Settings};
use benchmark::AdapterJsBenchmark;
use time::AdapterJsTime;
use vitest::AdapterJsVitest;

pub struct AdapterJs;

impl Adaptable for AdapterJs {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        AdapterJsBenchmark::parse(input, settings)
            .or_else(|| AdapterJsTime::parse(input, settings))
            .or_else(|| AdapterJsVitest::parse(input, settings))
    }
}

#[cfg(test)]
mod test_js {
    use super::{AdapterJs, time::test_js_time, vitest::test_js_vitest};
    use crate::adapters::{js::benchmark::test_js_benchmark, test_util::convert_file_path};

    #[test]
    fn adapter_js_benchmark() {
        let results = convert_file_path::<AdapterJs>("./tool_output/js/benchmark/four.txt");
        test_js_benchmark::validate_adapter_js_benchmark(&results);
    }

    #[test]
    fn adapter_js_time() {
        let results = convert_file_path::<AdapterJs>("./tool_output/js/time/four.txt");
        test_js_time::validate_adapter_js_time(&results);
    }

    #[test]
    fn adapter_js_vitest() {
        let results = convert_file_path::<AdapterJs>("./tool_output/js/vitest/four.json");
        test_js_vitest::validate_adapter_js_vitest(&results);
    }
}
