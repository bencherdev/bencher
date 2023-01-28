use bencher_json::JsonMetric;
use nom::{
    bytes::complete::tag,
    character::complete::{anychar, space1},
    combinator::{eof, map, map_res},
    multi::many_till,
    sequence::{delimited, tuple},
    IResult,
};
use ordered_float::OrderedFloat;

use crate::{
    adapters::util::{parse_f64, parse_units, time_as_nanos},
    results::adapter_results::AdapterResults,
    Adapter, AdapterError,
};

pub struct AdapterCppGoogle;

impl Adapter for AdapterCppGoogle {
    fn parse(input: &str) -> Result<AdapterResults, AdapterError> {
        serde_json::from_str(input).map_err(Into::into)
    }
}

#[cfg(test)]
pub(crate) mod test_rust_criterion {
    use bencher_json::JsonMetric;
    use pretty_assertions::assert_eq;

    use crate::{
        adapters::test_util::{convert_file_path, validate_metrics},
        Adapter, AdapterResults,
    };

    use super::AdapterCppGoogle;

    fn convert_cpp_google(suffix: &str) -> AdapterResults {
        let file_path = format!("./tool_output/cpp/google/{}.txt", suffix);
        convert_file_path::<AdapterCppGoogle>(&file_path)
    }

    #[test]
    fn test_adapter_json_latency() {
        let results = convert_cpp_google("two");
        validate_adapter_cpp_google(results);
    }

    pub fn validate_adapter_cpp_google(results: AdapterResults) {
        assert_eq!(results.inner.len(), 3);

        let metrics = results.get("fib_10").unwrap();
        validate_metrics(metrics, 214.98980114547953, None, None);

        let metrics = results.get("fib_20").unwrap();
        validate_metrics(metrics, 27_455.600415007055, None, None);
    }
}
