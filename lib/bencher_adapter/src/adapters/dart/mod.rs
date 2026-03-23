pub mod benchmark_harness;

use crate::{Adaptable, AdapterResults, Settings};
use benchmark_harness::AdapterDartBenchmarkHarness;

pub struct AdapterDart;

impl Adaptable for AdapterDart {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        AdapterDartBenchmarkHarness::parse(input, settings)
    }
}

#[cfg(test)]
mod test_dart {
    use super::AdapterDart;
    use crate::adapters::{
        dart::benchmark_harness::test_dart_benchmark_harness, test_util::convert_file_path,
    };

    #[test]
    fn adapter_dart() {
        let results =
            convert_file_path::<AdapterDart>("./tool_output/dart/benchmark_harness/two.txt");
        test_dart_benchmark_harness::validate_adapter_dart_benchmark_harness(&results);
    }
}
