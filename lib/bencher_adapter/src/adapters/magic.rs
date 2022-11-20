use nom::branch::alt;

use crate::{results::adapter_results::AdapterResults, Adapter, AdapterError, Settings};

use super::{json::parse_json, rust::parse_rust};

pub struct AdapterMagic;

impl Adapter for AdapterMagic {
    fn parse(input: &str, settings: Settings) -> Result<AdapterResults, AdapterError> {
        alt((parse_json, |i| parse_rust(i, settings)))(input)
            .map(|(_, benchmarks)| benchmarks)
            .map_err(|err| AdapterError::Nom(err.map_input(Into::into)))
    }
}

#[cfg(test)]
mod test {
    use super::AdapterMagic;
    use crate::{
        adapters::{json::test_json, rust::test_rust, test_util::convert_file_path},
        Settings,
    };

    #[test]
    fn test_adapter_magic_json_latency() {
        let results = convert_file_path::<AdapterMagic>(
            "./tool_output/json/report_latency.json",
            Settings::default(),
        );
        test_json::validate_adapter_json_latency(results);
    }

    #[test]
    fn test_adapter_magic_rust_many() {
        let results = convert_file_path::<AdapterMagic>(
            "./tool_output/rust/cargo_bench_many.txt",
            Settings::default(),
        );
        test_rust::validate_adapter_rust_many(results);
    }
}
