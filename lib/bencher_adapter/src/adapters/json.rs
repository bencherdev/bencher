use crate::{results::adapter_results::AdapterResults, Adaptable, Settings};

pub struct AdapterJson;

impl Adaptable for AdapterJson {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        serde_json::from_str(input).ok()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
pub(crate) mod test_json {
    use pretty_assertions::assert_eq;

    use super::AdapterJson;
    use crate::{
        adapters::test_util::{convert_file_path, validate_latency},
        results::adapter_results::AdapterResults,
    };

    fn convert_json(suffix: &str) -> AdapterResults {
        let file_path = format!("./tool_output/json/report_{suffix}.json");
        convert_file_path::<AdapterJson>(&file_path)
    }

    #[test]
    fn test_adapter_json_latency() {
        let results = convert_json("latency");
        validate_adapter_json_latency(&results);
    }

    pub fn validate_adapter_json_latency(results: &AdapterResults) {
        assert_eq!(results.inner.len(), 3);

        let metrics = results.get("tests::benchmark_a").unwrap();
        validate_latency(metrics, 3247.0, Some(1044.0), Some(1044.0));

        let metrics = results.get("tests::benchmark_b").unwrap();
        validate_latency(metrics, 3443.0, Some(2275.0), Some(2275.0));

        let metrics = results.get("tests::benchmark_c").unwrap();
        validate_latency(metrics, 3361.0, Some(1093.0), Some(1093.0));
    }
}
