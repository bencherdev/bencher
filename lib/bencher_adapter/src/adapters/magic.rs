use crate::{
    results::adapter_results::AdapterResults, Adapter, AdapterCpp, AdapterError, AdapterJson,
    AdapterRust,
};

pub struct AdapterMagic;

impl Adapter for AdapterMagic {
    fn parse(input: &str) -> Result<AdapterResults, AdapterError> {
        let json = AdapterJson::parse(input);
        if json.is_ok() {
            return json;
        }

        let rust = AdapterRust::parse(input)?;
        if !rust.is_empty() {
            return Ok(rust);
        }

        let cpp = AdapterCpp::parse(input)?;
        if !cpp.is_empty() {
            return Ok(cpp);
        }

        Ok(AdapterResults::default())
    }
}

#[cfg(test)]
mod test {
    use super::AdapterMagic;
    use crate::adapters::{
        json::test_json, rust::bench::test_rust_bench, test_util::convert_file_path,
    };

    #[test]
    fn test_adapter_magic_json_latency() {
        let results = convert_file_path::<AdapterMagic>("./tool_output/json/report_latency.json");
        test_json::validate_adapter_json_latency(results);
    }

    #[test]
    fn test_adapter_magic_rust_many() {
        let results = convert_file_path::<AdapterMagic>("./tool_output/rust/bench/many.txt");
        test_rust_bench::validate_adapter_rust_many(results);
    }
}
