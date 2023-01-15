use crate::{results::adapter_results::AdapterResults, Adapter, AdapterError, Settings};

pub struct AdapterJson;

impl Adapter for AdapterJson {
    fn parse(input: &str, settings: Settings) -> Result<AdapterResults, AdapterError> {
        let json = serde_json::from_str(input).map_err(Into::into);

        if json.is_err() && settings.allow_failure {
            Ok(AdapterResults::default())
        } else {
            json
        }
    }
}

#[cfg(test)]
pub(crate) mod test_json {
    use pretty_assertions::assert_eq;

    use super::AdapterJson;
    use crate::{
        adapters::test_util::{convert_file_path, validate_metrics},
        results::adapter_results::AdapterResults,
        Settings,
    };

    fn convert_json(suffix: &str, settings: Settings) -> AdapterResults {
        let file_path = format!("./tool_output/json/report_{}.json", suffix);
        convert_file_path::<AdapterJson>(&file_path, settings)
    }

    #[test]
    fn test_adapter_json_latency() {
        let results = convert_json("latency", Settings::default());
        validate_adapter_json_latency(results);
    }

    pub fn validate_adapter_json_latency(results: AdapterResults) {
        assert_eq!(results.inner.len(), 3);

        let metrics = results.get("tests::benchmark_a").unwrap();
        validate_metrics(metrics, 3247.0, Some(1044.0), Some(1044.0));

        let metrics = results.get("tests::benchmark_b").unwrap();
        validate_metrics(metrics, 3443.0, Some(2275.0), Some(2275.0));

        let metrics = results.get("tests::benchmark_c").unwrap();
        validate_metrics(metrics, 3361.0, Some(1093.0), Some(1093.0));
    }
}
