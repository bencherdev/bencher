use bencher_json::{JsonEmpty, JsonMetric, NonEmpty};
use nom::{
    bytes::complete::tag,
    character::complete::{anychar, space1},
    combinator::{eof, map, map_res},
    multi::many_till,
    sequence::{delimited, tuple},
    IResult,
};
use ordered_float::OrderedFloat;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::{
    adapters::util::{parse_f64, parse_units, time_as_nanos, Units},
    results::adapter_results::AdapterResults,
    Adapter, AdapterError,
};

pub struct AdapterCppGoogle;

impl Adapter for AdapterCppGoogle {
    fn parse(input: &str) -> Result<AdapterResults, AdapterError> {
        serde_json::from_str::<Google>(input)?.try_into()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Google {
    pub context: Context,
    pub benchmarks: Vec<Benchmark>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Context {
    pub caches: Vec<JsonEmpty>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Benchmark {
    pub name: NonEmpty,
    #[serde(with = "rust_decimal::serde::float")]
    pub real_time: Decimal,
    pub time_unit: Units,
}

impl TryFrom<Google> for AdapterResults {
    type Error = AdapterError;

    fn try_from(google: Google) -> Result<Self, Self::Error> {
        let mut benchmark_metrics = Vec::with_capacity(google.benchmarks.len());
        for benchmark in google.benchmarks {
            let Benchmark {
                name,
                real_time,
                time_unit,
            } = benchmark;
            let value = time_as_nanos(real_time, time_unit);
            let json_metric = JsonMetric {
                value,
                lower_bound: None,
                upper_bound: None,
            };

            benchmark_metrics.push((name.to_string(), json_metric));
        }

        benchmark_metrics.try_into()
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
