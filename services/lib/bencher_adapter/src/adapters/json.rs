use bencher_json::project::report::new::AdapterResults;
use nom::{
    character::complete::anychar,
    combinator::{eof, map_res},
    multi::many_till,
    IResult,
};

use crate::{Adapter, AdapterError};

pub struct AdapterJson;

impl Adapter for AdapterJson {
    fn convert(input: &str) -> Result<AdapterResults, AdapterError> {
        parse_json(input)
            .map(|(_, benchmarks)| benchmarks)
            .map_err(|err| AdapterError::Nom(err.map_input(Into::into)))
    }
}

pub fn parse_json(input: &str) -> IResult<&str, AdapterResults> {
    map_res(many_till(anychar, eof), |(char_array, _)| {
        serde_json::from_slice(&char_array.into_iter().map(|c| c as u8).collect::<Vec<u8>>())
    })(input)
}

#[cfg(test)]
pub(crate) mod test_json {
    use bencher_json::project::report::new::AdapterResults;
    use pretty_assertions::assert_eq;

    use super::AdapterJson;
    use crate::adapters::test_util::{convert_file_path, validate_metrics};

    fn convert_json(suffix: &str) -> AdapterResults {
        let file_path = format!("./tool_output/json/report_{}.json", suffix);
        convert_file_path::<AdapterJson>(&file_path)
    }

    #[test]
    fn test_adapter_json_latency() {
        let benchmarks_map = convert_json("latency");
        validate_adapter_json_latency(benchmarks_map);
    }

    pub fn validate_adapter_json_latency(benchmarks_map: AdapterResults) {
        assert_eq!(benchmarks_map.inner.len(), 3);

        let metrics = benchmarks_map.inner.get("tests::benchmark_a").unwrap();
        validate_metrics(metrics, 3247.0, Some(1044.0), Some(1044.0));

        let metrics = benchmarks_map.inner.get("tests::benchmark_b").unwrap();
        validate_metrics(metrics, 3443.0, Some(2275.0), Some(2275.0));

        let metrics = benchmarks_map.inner.get("tests::benchmark_c").unwrap();
        validate_metrics(metrics, 3361.0, Some(1093.0), Some(1093.0));
    }
}
