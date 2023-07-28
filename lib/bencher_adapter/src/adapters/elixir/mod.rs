pub mod benchee;

use crate::{Adapter, AdapterResults, Settings};

use benchee::AdapterElixirBenchee;

pub struct AdapterElixir;

impl Adapter for AdapterElixir {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        AdapterElixirBenchee::parse(input, settings)
    }
}

#[cfg(test)]
mod test_go {
    use super::AdapterGo;
    use crate::adapters::{go::bench::test_go_bench, test_util::convert_file_path};

    #[test]
    fn test_adapter_elixir_benchee() {
        let results = convert_file_path::<AdapterElixir>("./tool_output/elixir/benchee/five.txt");
        test_go_bench::validate_adapter_go_bench(results);
    }
}
