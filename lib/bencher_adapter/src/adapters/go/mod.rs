pub mod bench;

use crate::{Adaptable, AdapterResults, Settings};
use bench::AdapterGoBench;

pub struct AdapterGo;

impl Adaptable for AdapterGo {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        AdapterGoBench::parse(input, settings)
    }
}

#[cfg(test)]
mod test_go {
    use super::AdapterGo;
    use crate::adapters::{go::bench::test_go_bench, test_util::convert_file_path};

    #[test]
    fn test_adapter_go_bench() {
        let results = convert_file_path::<AdapterGo>("./tool_output/go/bench/five.txt");
        test_go_bench::validate_adapter_go_bench(&results);
    }
}
