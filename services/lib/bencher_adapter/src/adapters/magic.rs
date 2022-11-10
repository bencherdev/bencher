use bencher_json::project::report::new::AdapterResults;
use nom::branch::alt;

use crate::{Adapter, AdapterError};

use super::{json::parse_json, rust::parse_rust};

pub struct AdapterMagic;

impl Adapter for AdapterMagic {
    fn convert(input: &str) -> Result<AdapterResults, AdapterError> {
        alt((parse_json, parse_rust))(input)
            .map(|(_, benchmarks)| benchmarks)
            .map_err(|err| AdapterError::Nom(err.map_input(Into::into)))
    }
}

#[cfg(test)]
mod test {

    use super::AdapterMagic;
    use crate::adapters::{json::test_json, rust::test_rust, test_util::convert_file_path};

    #[test]
    fn test_adapter_magic_json_latency() {
        let benchmarks_map =
            convert_file_path::<AdapterMagic>("./tool_output/json/report_latency.json");
        test_json::validate_adapter_json_latency(benchmarks_map);
    }

    #[test]
    fn test_adapter_magic_rust_many() {
        let benchmarks_map =
            convert_file_path::<AdapterMagic>("./tool_output/rust/cargo_bench_many.txt");
        test_rust::validate_adapter_rust_many(benchmarks_map);
    }
}
