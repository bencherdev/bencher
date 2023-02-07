pub mod benchmark;

use crate::{Adapter, AdapterResults, Settings};
use benchmark::AdapterRubyBenchmark;

pub struct AdapterRuby;

impl Adapter for AdapterRuby {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        AdapterRubyBenchmark::parse(input, settings)
    }
}

#[cfg(test)]
mod test_go {
    use super::AdapterRuby;
    use crate::adapters::{ruby::benchmark::test_ruby_benchmark, test_util::convert_file_path};

    #[test]
    fn test_adapter_ruby_benchmark() {
        let results = convert_file_path::<AdapterRuby>("./tool_output/ruby/benchmark/five.txt");
        test_ruby_benchmark::validate_adapter_ruby_benchmark(results);
    }
}
