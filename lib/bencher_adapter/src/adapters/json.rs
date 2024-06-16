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
    use ordered_float::OrderedFloat;
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

    #[test]
    fn test_adapter_json_dhat() {
        let results = convert_json("dhat");
        validate_adapter_json_dhat(&results);
    }

    pub fn validate_adapter_json_dhat(results: &AdapterResults) {
        assert_eq!(results.inner.len(), 1);
        let metrics = results.get("bench_play_game").unwrap();
        assert_eq!(metrics.inner.len(), 6);
        for (key, value) in [
            ("Final Blocks", 0.0),
            ("Final Bytes", 0.0),
            ("Max Blocks", 1.0),
            ("Max Bytes", 9.0),
            ("Total Blocks", 100.0),
            ("Total Bytes", 662.0),
        ] {
            let metric = metrics.get(key).unwrap();
            assert_eq!(metric.value, OrderedFloat::from(value));
            assert_eq!(metric.lower_value, None);
            assert_eq!(metric.upper_value, None);
        }
    }
}
