pub mod bench;

use crate::{Adapter, AdapterError, AdapterResults};
use bench::AdapterGoBench;

pub struct AdapterGo;

impl Adapter for AdapterGo {
    fn parse(input: &str) -> Result<AdapterResults, AdapterError> {
        let bench = AdapterGoBench::parse(input)?;
        if !bench.is_empty() {
            return Ok(bench);
        }

        Ok(AdapterResults::default())
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
