pub mod bench;

use crate::{Adapter, AdapterResults};
use bench::AdapterGoBench;

pub struct AdapterGo;

impl Adapter for AdapterGo {
    fn parse(input: &str) -> Option<AdapterResults> {
        AdapterGoBench::parse(input)
    }
}

#[cfg(test)]
mod test_go {
    use super::AdapterGo;
    use crate::adapters::{go::bench::test_go_bench, test_util::convert_file_path};

    #[test]
    fn test_adapter_go_bench() {
        let results = convert_file_path::<AdapterGo>("./tool_output/go/bench/five.txt");
        test_go_bench::validate_adapter_go_bench(results);
    }
}
