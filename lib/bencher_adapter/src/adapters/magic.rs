use crate::{
    results::adapter_results::AdapterResults, Adapter, AdapterError, AdapterJson, AdapterRust,
    Settings,
};

pub struct AdapterMagic;

impl Adapter for AdapterMagic {
    fn parse(input: &str, settings: Settings) -> Result<AdapterResults, AdapterError> {
        let json = AdapterJson::parse(input, settings);
        if json.is_ok() {
            return json;
        }

        let rust = AdapterRust::parse(input, settings)?;
        if !rust.is_empty() {
            return Ok(rust);
        }

        Ok(AdapterResults::default())
    }
}

#[cfg(test)]
mod test {
    use super::AdapterMagic;
    use crate::{
        adapters::{json::test_json, rust::bench::test_rust, test_util::convert_file_path},
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
            "./tool_output/rust/bench/many.txt",
            Settings::default(),
        );
        test_rust::validate_adapter_rust_many(results);
    }
}
